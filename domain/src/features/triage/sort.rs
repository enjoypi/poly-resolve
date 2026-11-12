use alloc::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, from_str};

use super::pack::layout::{Choice, Layout, Segment};
use crate::AppError;

const CHOICE: &str = "choice";
const ONE: f64 = 1.0;
const PREDICATE: &str = "predicate";
const REFUSAL: &str = "refusal";
const ZERO: f64 = 0.0;

#[derive(Deserialize)]
struct Answer {
    choice: Option<Value>,
    confidence: Option<f64>,
    #[serde(rename = "type")]
    kind: String,
    probability: Option<f64>,
}

struct Group {
    label: String,
    last_ms: u64,
    probability: Option<f64>,
    score: Option<f64>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriagePlacement")]
pub(crate) struct Placement {
    key: String,
    probability: Option<f64>,
    topic: Option<String>,
}

#[derive(Deserialize)]
struct Reply {
    answers: Vec<Option<Answer>>,
}

struct Resolve<'resolve> {
    answers: &'resolve [Option<Answer>],
    min: f64,
}

impl Resolve<'_> {
    fn chosen(&self, segment: &Segment, done: &[Option<String>]) -> Option<String> {
        let answer = segment
            .question()
            .and_then(|index| self.answers.get(index))
            .and_then(Option::as_ref)?;
        let value = (answer.kind == CHOICE
            && answer.confidence.is_some_and(|score| score >= self.min))
        .then_some(answer.choice.as_ref())
        .flatten()
        .and_then(Value::as_str)?;
        let pick = Choice::find(segment.choices(), value)?;
        pick.topic().map(str::to_owned).or_else(|| {
            let before = pick.segment()?;
            done.get(before).cloned().flatten()
        })
    }

    fn topics(&self, layout: &Layout) -> Vec<Option<String>> {
        layout.segments().iter().enumerate().fold(
            Vec::new(),
            |mut done: Vec<Option<String>>, (index, segment)| {
                let topic = segment
                    .attach()
                    .map(str::to_owned)
                    .or_else(|| self.chosen(segment, &done))
                    .or_else(|| {
                        segment
                            .role()
                            .is_empty()
                            .then(|| format!("t{}-{index}", layout.batch_id()))
                    });
                done.push(topic);
                done
            },
        )
    }
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageSortRequest")]
pub(crate) struct SortRequest {
    layout: Layout,
    min_confidence: f64,
    response: String,
    threshold: f64,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriageSortResponse")]
pub(crate) struct SortResponse {
    placements: Vec<Placement>,
    topics: Vec<Touched>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriageTouched")]
pub(crate) struct Touched {
    label: String,
    last_ms: u64,
    probability: Option<f64>,
    score: Option<f64>,
    topic: String,
    urgent: Verdict,
}

#[derive(Clone, Copy, JsonSchema, Serialize)]
#[schemars(rename = "TriageVerdict")]
#[serde(rename_all = "lowercase")]
pub(crate) enum Verdict {
    Normal,
    Urgent,
}

fn check(reply: &Reply, layout: &Layout) -> Result<(), AppError> {
    if reply.answers.len() != layout.questions() {
        return Err(AppError::BadRequest);
    }
    let line_count = layout.predicates();
    for (index, answer) in reply.answers.iter().enumerate() {
        let expected = if index < line_count {
            PREDICATE
        } else {
            CHOICE
        };
        if let Some(found) = answer.as_ref()
            && found.kind != expected
            && found.kind != REFUSAL
        {
            return Err(AppError::BadRequest);
        }
    }
    Ok(())
}

fn grouped(layout: &Layout, placements: &[Placement], threshold: f64) -> Vec<Touched> {
    let mut groups: BTreeMap<&str, Group> = BTreeMap::new();
    for (line, placement) in layout.lines().iter().zip(placements) {
        let Some(topic) = placement.topic.as_deref() else {
            continue;
        };
        let group = groups.entry(topic).or_insert_with(|| Group {
            label: layout
                .segments()
                .get(line.segment())
                .map_or_default(|segment| segment.label().to_owned()),
            last_ms: 0,
            probability: layout.probabilities().get(topic).copied().flatten(),
            score: None,
        });
        group.last_ms = group.last_ms.max(line.occurred_at_ms());
        group.probability = group
            .probability
            .into_iter()
            .chain(placement.probability)
            .reduce(f64::max);
        group.score = group
            .score
            .into_iter()
            .chain(placement.probability)
            .reduce(f64::max);
    }
    groups
        .into_iter()
        .map(|(topic, group)| Touched {
            label: group.label,
            last_ms: group.last_ms,
            probability: group.probability,
            score: group.score,
            topic: topic.to_owned(),
            urgent: if group.score.is_some_and(|value| value >= threshold) {
                Verdict::Urgent
            } else {
                Verdict::Normal
            },
        })
        .collect()
}

fn probability(answer: &Answer) -> Option<f64> {
    (answer.kind == PREDICATE)
        .then_some(answer.probability)
        .flatten()
        .filter(|value| (ZERO..=ONE).contains(value))
}

pub(crate) fn sort(req: &SortRequest) -> Result<SortResponse, AppError> {
    let reply: Reply = from_str(&req.response).map_err(|_err| AppError::BadRequest)?;
    let layout = &req.layout;
    check(&reply, layout)?;
    let resolved = Resolve {
        answers: &reply.answers,
        min: req.min_confidence,
    }
    .topics(layout);
    let mut asked: usize = 0;
    let placements: Vec<Placement> = layout
        .lines()
        .iter()
        .map(|line| {
            let scored = line.role().is_empty().then(|| {
                let slot = asked;
                asked = asked.saturating_add(1);
                slot
            });
            Placement {
                key: line.key().to_owned(),
                probability: scored
                    .and_then(|slot| reply.answers.get(slot))
                    .and_then(Option::as_ref)
                    .and_then(probability),
                topic: resolved.get(line.segment()).cloned().flatten(),
            }
        })
        .collect();
    let topics = grouped(layout, &placements, req.threshold);
    Ok(SortResponse { placements, topics })
}
