use alloc::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json, to_string};

use super::redact::{Mask, Rule};
use super::transcript::{Row, Style};
use crate::AppError;

const LINE: &str = "{line}";
pub(super) const NEW: &str = "new";
const SEGMENT: &str = "{segment}";

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageAttach")]
pub(crate) struct Attach {
    conversation_id: String,
    last_ms: u64,
    #[serde(default)]
    probability: Option<f64>,
    topic_id: String,
}

struct Block<'block> {
    attach: Option<&'block str>,
    entries: Vec<&'block Entry>,
}

impl Block<'_> {
    fn first_ms(&self) -> u64 {
        self.entries
            .first()
            .map_or(0, |entry| entry.row.occurred_at_ms())
    }
}

#[derive(Clone, Deserialize, JsonSchema, Serialize)]
#[schemars(rename = "TriageChoice")]
pub(crate) struct Choice {
    segment: Option<usize>,
    topic: Option<String>,
    value: String,
}

impl Choice {
    pub(super) fn find<'choice>(choices: &'choice [Self], value: &str) -> Option<&'choice Self> {
        choices.iter().find(|choice| choice.value == value)
    }

    pub(super) const fn segment(&self) -> Option<usize> {
        self.segment
    }

    pub(super) fn topic(&self) -> Option<&str> {
        self.topic.as_deref()
    }
}

struct Draft {
    input: Vec<Value>,
    lines: Vec<Placed>,
    urgent: Vec<Value>,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageEntry")]
pub(crate) struct Entry {
    conversation_id: String,
    #[serde(default)]
    hints: Vec<String>,
    key: String,
    #[serde(flatten)]
    row: Row,
}

#[derive(Deserialize, JsonSchema, Serialize)]
#[schemars(rename = "TriageLayout")]
pub(crate) struct Layout {
    batch_id: u64,
    lines: Vec<Placed>,
    #[serde(default)]
    probabilities: BTreeMap<String, Option<f64>>,
    segments: Vec<Segment>,
}

impl Layout {
    pub(super) const fn batch_id(&self) -> u64 {
        self.batch_id
    }

    pub(super) fn lines(&self) -> &[Placed] {
        &self.lines
    }

    pub(super) const fn probabilities(&self) -> &BTreeMap<String, Option<f64>> {
        &self.probabilities
    }

    pub(super) fn questions(&self) -> usize {
        self.lines.len().saturating_add(
            self.segments
                .iter()
                .filter(|segment| segment.question.is_some())
                .count(),
        )
    }

    pub(super) fn segments(&self) -> &[Segment] {
        &self.segments
    }
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageOpen")]
pub(crate) struct Open {
    label: String,
    #[serde(default)]
    probability: Option<f64>,
    topic: String,
}

impl Open {
    pub(super) fn label(&self) -> &str {
        &self.label
    }

    pub(super) fn topic(&self) -> &str {
        &self.topic
    }
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriagePackRequest")]
pub(crate) struct PackRequest {
    attach: Vec<Attach>,
    batch_id: u64,
    gap_ms: u64,
    label_chars: u32,
    lines: Vec<Entry>,
    max_chars: u32,
    model: String,
    new_label: String,
    questions: Questions,
    rules: Vec<Rule>,
    topics: Vec<Open>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriagePackResponse")]
pub(crate) struct PackResponse {
    body: String,
    layout: Layout,
}

struct Pick {
    choice: Choice,
    text: String,
}

type Picks = Vec<Pick>;

#[derive(Deserialize, JsonSchema, Serialize)]
#[schemars(rename = "TriagePlaced")]
pub(crate) struct Placed {
    key: String,
    occurred_at_ms: u64,
    segment: usize,
}

impl Placed {
    pub(super) fn key(&self) -> &str {
        &self.key
    }

    pub(super) const fn occurred_at_ms(&self) -> u64 {
        self.occurred_at_ms
    }

    pub(super) const fn segment(&self) -> usize {
        self.segment
    }
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageQuestion")]
pub(crate) struct Question {
    instructions: String,
    name: String,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageQuestions")]
pub(crate) struct Questions {
    topic: Question,
    urgent: Question,
}

#[derive(Deserialize, JsonSchema, Serialize)]
#[schemars(rename = "TriageSegment")]
pub(crate) struct Segment {
    attach: Option<String>,
    choices: Vec<Choice>,
    label: String,
    question: Option<usize>,
}

impl Segment {
    pub(super) fn attach(&self) -> Option<&str> {
        self.attach.as_deref()
    }

    pub(super) fn choices(&self) -> &[Choice] {
        &self.choices
    }

    pub(super) fn label(&self) -> &str {
        &self.label
    }

    pub(super) const fn question(&self) -> Option<usize> {
        self.question
    }
}

pub(super) fn alias(prefix: &str, index: usize) -> String {
    format!("{prefix}{}", index.saturating_add(1))
}

fn attached<'req>(req: &'req PackRequest, first: &Entry) -> Option<&'req str> {
    req.attach
        .iter()
        .find(|point| {
            point.conversation_id == first.conversation_id
                && first.row.occurred_at_ms().saturating_sub(point.last_ms) <= req.gap_ms
        })
        .map(|point| point.topic_id.as_str())
}

fn blocks(req: &PackRequest) -> Vec<Block<'_>> {
    let mut sorted: Vec<&Entry> = req.lines.iter().collect();
    sorted.sort_by_key(|entry| {
        (
            entry.conversation_id.as_str(),
            entry.row.occurred_at_ms(),
            entry.key.as_str(),
        )
    });
    let mut grouped: Vec<Vec<&Entry>> = Vec::new();
    for entry in sorted {
        let joined = grouped.last_mut().filter(|block| {
            block.last().is_some_and(|prev| {
                prev.conversation_id == entry.conversation_id
                    && entry
                        .row
                        .occurred_at_ms()
                        .saturating_sub(prev.row.occurred_at_ms())
                        <= req.gap_ms
            })
        });
        match joined {
            Some(block) => block.push(entry),
            None => grouped.push(vec![entry]),
        }
    }
    let mut found: Vec<Block<'_>> = grouped
        .into_iter()
        .map(|entries| Block {
            attach: entries.first().and_then(|first| attached(req, first)),
            entries,
        })
        .collect();
    found.sort_by_key(|block| {
        (
            block.first_ms(),
            block.entries.first().map(|entry| entry.key.as_str()),
        )
    });
    found
}

fn choice(question: &Question, index: usize, options: &[Pick]) -> Value {
    json!({
        "choices": options
            .iter()
            .map(|pick| json!({"description": pick.text, "value": pick.choice.value}))
            .collect::<Vec<Value>>(),
        "instructions": question.instructions.replace(SEGMENT, &alias("S", index)),
        "name": format!("{}_{}", question.name, index.saturating_add(1)),
        "type": "choice",
    })
}

fn draft(req: &PackRequest, found: &[Block<'_>], style: &Style<'_>) -> Draft {
    let mut seen: Vec<&str> = Vec::new();
    let mut done = Draft {
        input: Vec::new(),
        lines: Vec::new(),
        urgent: Vec::new(),
    };
    for (index, block) in found.iter().enumerate() {
        let mut shown: Vec<Value> = Vec::new();
        for entry in &block.entries {
            let number = done.lines.len();
            shown.push(labelled(&entry.row.render(style), &alias("L", number)));
            done.urgent.push(predicate(&req.questions.urgent, number));
            done.lines.push(Placed {
                key: entry.key.clone(),
                occurred_at_ms: entry.row.occurred_at_ms(),
                segment: index,
            });
        }
        let conversation = block
            .entries
            .first()
            .map_or("", |entry| entry.conversation_id.as_str());
        let slot = seen
            .iter()
            .position(|known| *known == conversation)
            .unwrap_or_else(|| {
                seen.push(conversation);
                seen.len().saturating_sub(1)
            });
        done.input.push(json!({
            "conversation": alias("C", slot),
            "lines": shown,
            "segment": alias("S", index),
        }));
    }
    done
}

fn label_of(req: &PackRequest, block: &Block<'_>, style: &Style<'_>) -> String {
    let chars = usize::try_from(req.label_chars).unwrap_or(usize::MAX);
    block.entries.first().map_or_else(String::new, |entry| {
        style
            .mask()
            .apply(entry.row.text())
            .chars()
            .take(chars)
            .collect()
    })
}

fn labelled(shown: &Value, tag: &str) -> Value {
    let mut map = shown.as_object().cloned().unwrap_or_default();
    drop(map.insert("line".to_owned(), Value::String(tag.to_owned())));
    Value::Object(map)
}

fn options(req: &PackRequest, found: &[Block<'_>], style: &Style<'_>) -> Vec<Picks> {
    found
        .iter()
        .enumerate()
        .map(|(index, block)| {
            let narrowed: Vec<&str> = block
                .entries
                .iter()
                .flat_map(|entry| entry.hints.iter().map(String::as_str))
                .collect();
            let known = req
                .topics
                .iter()
                .filter(|open| narrowed.is_empty() || narrowed.contains(&open.topic.as_str()))
                .map(|open| Pick {
                    choice: Choice {
                        segment: None,
                        topic: Some(open.topic.clone()),
                        value: open.topic.clone(),
                    },
                    text: style.mask().apply(&open.label),
                });
            let earlier = found
                .iter()
                .take(index)
                .enumerate()
                .map(|(before, other)| Pick {
                    choice: Choice {
                        segment: Some(before),
                        topic: None,
                        value: alias("S", before),
                    },
                    text: label_of(req, other, style),
                });
            let fresh = Pick {
                choice: Choice {
                    segment: None,
                    topic: None,
                    value: NEW.to_owned(),
                },
                text: req.new_label.clone(),
            };
            let all: Picks = known.chain(earlier).chain([fresh]).collect();
            if block.attach.is_some() || all.len() < 2 {
                Vec::new()
            } else {
                all
            }
        })
        .collect()
}

pub(crate) fn pack(req: &PackRequest) -> Result<PackResponse, AppError> {
    let mask = Mask::new(&req.rules)?;
    let max = usize::try_from(req.max_chars).unwrap_or(usize::MAX);
    let style = Style::new(&mask, max);
    let found = blocks(req);
    let drafted = draft(req, &found, &style);
    let options = options(req, &found, &style);
    let mut next = drafted.lines.len();
    let segments: Vec<Segment> = found
        .iter()
        .zip(&options)
        .map(|(block, picks)| {
            let question = (!picks.is_empty()).then(|| {
                let slot = next;
                next = next.saturating_add(1);
                slot
            });
            Segment {
                attach: block.attach.map(str::to_owned),
                choices: picks.iter().map(|pick| pick.choice.clone()).collect(),
                label: label_of(req, block, &style),
                question,
            }
        })
        .collect();
    let asked = options
        .iter()
        .enumerate()
        .filter(|&(_, picks)| !picks.is_empty())
        .map(|(index, picks)| choice(&req.questions.topic, index, picks));
    let questions: Vec<Value> = drafted.urgent.into_iter().chain(asked).collect();
    let body = json!({
        "input": Value::Array(drafted.input).to_string(),
        "model": req.model,
        "questions": questions,
    });
    Ok(PackResponse {
        body: to_string(&body).unwrap_or_default(),
        layout: Layout {
            batch_id: req.batch_id,
            lines: drafted.lines,
            probabilities: req
                .topics
                .iter()
                .map(|topic| (topic.topic.clone(), topic.probability))
                .chain(
                    req.attach
                        .iter()
                        .map(|point| (point.topic_id.clone(), point.probability)),
                )
                .collect(),
            segments,
        },
    })
}

fn predicate(question: &Question, number: usize) -> Value {
    json!({
        "instructions": question.instructions.replace(LINE, &alias("L", number)),
        "name": format!("{}_{}", question.name, number.saturating_add(1)),
        "type": "predicate",
    })
}
