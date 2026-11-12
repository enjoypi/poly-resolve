use alloc::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, from_str};

use super::pack::layout::{Choice, Layout, Placed, Segment};
use crate::AppError;

const ADJACENT: &str = "adjacent";
const CHOICE: &str = "choice";
const CLUE: &str = "clue";
const NEW: &str = "new";
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
}

struct Home {
    topic: String,
    via: &'static str,
}

impl Home {
    fn new(topic: &str, via: &'static str) -> Self {
        Self {
            topic: topic.to_owned(),
            via,
        }
    }
}

type Pick = (Vec<String>, Option<f64>);

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriagePlacement")]
pub(crate) struct Placement {
    key: String,
    probability: Option<f64>,
    topic: Option<String>,
    via: Option<String>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriageRelated")]
pub(crate) struct Related {
    fresh: String,
    known: String,
}

#[derive(Deserialize)]
struct Reply {
    answers: Vec<Option<Answer>>,
}

struct Resolve<'resolve> {
    answers: &'resolve [Option<Answer>],
    layout: &'resolve Layout,
    min: f64,
}

impl Resolve<'_> {
    fn asked(&self, index: usize, found: &mut Resolved) -> Vec<Home> {
        let (targets, confidence) = self
            .layout
            .segments()
            .get(index)
            .and_then(|segment| self.picked(segment, &found.homes))
            .unwrap_or_default();
        if !targets.is_empty() && confidence.is_some_and(|score| score >= self.min) {
            return targets
                .iter()
                .map(|topic| Home::new(topic, CHOICE))
                .collect();
        }
        let fresh: Vec<Home> = self
            .layout
            .segments()
            .get(index)
            .filter(|segment| segment.role().is_empty())
            .map(|_| Home::new(&format!("t{}-{index}", self.layout.batch_id()), NEW))
            .into_iter()
            .collect();
        for home in &fresh {
            found.relations.extend(targets.iter().map(|target| Related {
                fresh: home.topic.clone(),
                known: target.clone(),
            }));
        }
        fresh
    }

    fn homes(&self) -> Resolved {
        let mut found = Resolved {
            homes: Vec::new(),
            relations: Vec::new(),
        };
        for (index, segment) in self.layout.segments().iter().enumerate() {
            let fixed: Vec<Home> = segment
                .attach()
                .map(|topic| Home::new(topic, ADJACENT))
                .into_iter()
                .chain(
                    segment
                        .linked()
                        .iter()
                        .filter(|topic| segment.attach() != Some(topic.as_str()))
                        .map(|topic| Home::new(topic, CLUE)),
                )
                .collect();
            let homes = if fixed.is_empty() {
                self.asked(index, &mut found)
            } else {
                fixed
            };
            found.homes.push(homes);
        }
        found
    }

    fn picked(&self, segment: &Segment, done: &[Vec<Home>]) -> Option<Pick> {
        let answer = segment
            .question()
            .and_then(|index| self.answers.get(index))
            .and_then(Option::as_ref)?;
        let value = (answer.kind == CHOICE)
            .then_some(answer.choice.as_ref())
            .flatten()
            .and_then(Value::as_str)?;
        let pick = Choice::find(segment.choices(), value)?;
        let targets = pick.topic().map_or_else(
            || {
                pick.segment()
                    .and_then(|before| done.get(before))
                    .map_or_default(|homes| homes.iter().map(|home| home.topic.clone()).collect())
            },
            |topic| vec![topic.to_owned()],
        );
        Some((targets, answer.confidence))
    }
}

struct Resolved {
    homes: Vec<Vec<Home>>,
    relations: Vec<Related>,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageSortRequest")]
pub(crate) struct SortRequest {
    layout: Layout,
    min_confidence: f64,
    response: String,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriageSortResponse")]
pub(crate) struct SortResponse {
    placements: Vec<Placement>,
    relations: Vec<Related>,
    reviews: Vec<Verdict>,
    topics: Vec<Touched>,
}

struct Tally<'tally> {
    groups: BTreeMap<String, Group>,
    layout: &'tally Layout,
    placements: Vec<Placement>,
}

impl Tally<'_> {
    fn add(&mut self, line: &Placed, spot: (&Home, Option<f64>)) {
        let (home, probability) = spot;
        let layout = self.layout;
        let group = self
            .groups
            .entry(home.topic.clone())
            .or_insert_with(|| Group {
                label: layout
                    .segments()
                    .get(line.segment())
                    .map_or_default(|segment| segment.label().to_owned()),
                last_ms: 0,
                probability: layout.probabilities().get(&home.topic).copied().flatten(),
            });
        group.last_ms = group.last_ms.max(line.occurred_at_ms());
        group.probability = group
            .probability
            .into_iter()
            .chain(probability)
            .reduce(f64::max);
        self.placements.push(Placement {
            key: line.key().to_owned(),
            probability,
            topic: Some(home.topic.clone()),
            via: Some(home.via.to_owned()),
        });
    }

    fn finish(self, relations: Vec<Related>, reviews: Vec<Verdict>) -> SortResponse {
        SortResponse {
            placements: self.placements,
            relations,
            reviews,
            topics: self
                .groups
                .into_iter()
                .map(|(topic, group)| Touched {
                    label: group.label,
                    last_ms: group.last_ms,
                    probability: group.probability,
                    topic,
                })
                .collect(),
        }
    }
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriageTouched")]
pub(crate) struct Touched {
    label: String,
    last_ms: u64,
    probability: Option<f64>,
    topic: String,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriageVerdict")]
pub(crate) struct Verdict {
    probability: Option<f64>,
    tag: Value,
}

fn check(reply: &Reply, layout: &Layout) -> Result<(), AppError> {
    if reply.answers.len() != layout.questions() {
        return Err(AppError::BadRequest);
    }
    let line_count = layout.scored();
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
        layout,
        min: req.min_confidence,
    }
    .homes();
    let mut asked: usize = 0;
    let mut tally = Tally {
        groups: BTreeMap::new(),
        layout,
        placements: Vec::new(),
    };
    for line in layout.lines() {
        let scored = line.role().is_empty().then(|| {
            let slot = asked;
            asked = asked.saturating_add(1);
            slot
        });
        let chance = scored
            .and_then(|slot| reply.answers.get(slot))
            .and_then(Option::as_ref)
            .and_then(probability);
        let homes = resolved
            .homes
            .get(line.segment())
            .map_or(&[][..], Vec::as_slice);
        if homes.is_empty() {
            tally.placements.push(Placement {
                key: line.key().to_owned(),
                probability: chance,
                topic: None,
                via: None,
            });
        }
        for home in homes {
            tally.add(line, (home, chance));
        }
    }
    let reviews = layout
        .reviews()
        .iter()
        .enumerate()
        .map(|(index, tag)| Verdict {
            probability: reply
                .answers
                .get(layout.predicates().saturating_add(index))
                .and_then(Option::as_ref)
                .and_then(probability),
            tag: tag.clone(),
        })
        .collect();
    Ok(tally.finish(resolved.relations, reviews))
}
