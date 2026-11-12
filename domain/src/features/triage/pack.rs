pub(super) mod layout;

use alloc::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json, to_string};

use self::layout::{Choice, Layout, Placed, Segment};
use super::redact::{Mask, Rule};
use super::transcript::{Row, Style};
use crate::AppError;
use crate::redact::Seal;

const AUTO: &str = "auto";
const COPY: &str = "copy";
pub(crate) const LINE: &str = "{line}";
const NEW: &str = "new";
const NONE: &str = "none";
pub(crate) const REVIEW: &str = "{review}";
pub(crate) const SEGMENT: &str = "{segment}";

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
    linked: Vec<&'block str>,
}

impl Block<'_> {
    fn auto(&self) -> bool {
        self.entries.iter().all(|entry| entry.row.role() == AUTO)
    }

    fn first_ms(&self) -> u64 {
        self.entries
            .first()
            .map_or(0, |entry| entry.row.occurred_at_ms())
    }
}

type Checks = (Vec<Value>, Vec<Value>);

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
    created_at_ms: u64,
    #[serde(default)]
    hints: Vec<String>,
    key: String,
    #[serde(flatten)]
    row: Row,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageLink")]
pub(crate) struct Link {
    key: String,
    topic: String,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageOpen")]
pub(crate) struct Open {
    #[serde(default)]
    created_at_ms: u64,
    #[serde(default)]
    first: Option<String>,
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
    #[serde(default)]
    linked: Vec<Link>,
    max_chars: u32,
    model: String,
    new_label: String,
    none_label: String,
    questions: Questions,
    #[serde(default)]
    reviews: Vec<Review>,
    rules: Vec<Rule>,
    topics: Vec<Open>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriagePackResponse")]
pub(crate) struct PackResponse {
    body: String,
    layout: Layout,
    questions: usize,
}

struct Pick {
    choice: Choice,
    text: String,
}

type Picks = Vec<Pick>;

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageQuestion")]
pub(crate) struct Question {
    instructions: String,
    name: String,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageQuestions")]
pub(crate) struct Questions {
    review: Question,
    topic: Question,
    urgent: Question,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageReview")]
pub(crate) struct Review {
    left: String,
    right: String,
    tag: Value,
}

pub(super) fn alias(prefix: &str, index: usize) -> String {
    format!("{prefix}{}", index.saturating_add(1))
}

fn attached<'req>(req: &'req PackRequest, first: &Entry) -> Option<&'req str> {
    req.attach
        .iter()
        .find(|point| {
            point.conversation_id == first.conversation_id
                && first.row.occurred_at_ms().abs_diff(point.last_ms) <= req.gap_ms
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
        .map(|entries| {
            let linked = linked(req, &entries);
            Block {
                attach: entries.first().and_then(|first| attached(req, first)),
                entries,
                linked,
            }
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
            .map(|pick| json!({"description": pick.text, "value": pick.choice.value()}))
            .collect::<Vec<Value>>(),
        "instructions": question.instructions.replace(SEGMENT, &alias("S", index)),
        "name": format!("{}_{}", question.name, index.saturating_add(1)),
        "type": "choice",
    })
}

fn draft(req: &PackRequest, found: &[Block<'_>], style: &Style<'_>) -> Draft {
    let mut seen: Vec<&str> = Vec::new();
    let mut first: BTreeMap<&str, usize> = BTreeMap::new();
    let mut done = Draft {
        input: Vec::new(),
        lines: Vec::new(),
        urgent: Vec::new(),
    };
    for (index, block) in found.iter().enumerate() {
        let mut shown: Vec<Value> = Vec::new();
        for entry in &block.entries {
            let number = done.lines.len();
            let digest = entry.row.digest();
            let origin = first.get(digest).copied().filter(|_| !digest.is_empty());
            let role = if let Some(before) = origin {
                shown.push(json!({"line": alias("L", number), "repeat": alias("L", before)}));
                COPY
            } else {
                let _: Option<usize> = first.insert(digest, number);
                shown.push(labelled(&entry.row.render(style), &alias("L", number)));
                entry.row.role()
            };
            if role.is_empty() {
                done.urgent.push(predicate(&req.questions.urgent, number));
            }
            done.lines.push(
                Placed::new(entry.key.clone(), entry.row.occurred_at_ms(), index).with_role(role),
            );
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

fn history(req: &PackRequest) -> BTreeMap<String, Option<f64>> {
    req.topics
        .iter()
        .map(|topic| (topic.topic.clone(), topic.probability))
        .chain(
            req.attach
                .iter()
                .map(|point| (point.topic_id.clone(), point.probability)),
        )
        .collect()
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

fn linked<'req>(req: &'req PackRequest, entries: &[&Entry]) -> Vec<&'req str> {
    let mut found: Vec<&str> = req
        .linked
        .iter()
        .filter(|link| entries.iter().any(|entry| entry.key == link.key))
        .map(|link| link.topic.as_str())
        .collect();
    found.sort_unstable();
    found.dedup();
    found
}

fn offered(req: &PackRequest, block: &Block<'_>, open: &Open) -> bool {
    let narrowed: Vec<&str> = block
        .entries
        .iter()
        .flat_map(|entry| entry.hints.iter().map(String::as_str))
        .filter(|hint| req.topics.iter().any(|known| known.topic == *hint))
        .collect();
    let earliest = block
        .entries
        .iter()
        .map(|entry| entry.created_at_ms)
        .min()
        .unwrap_or_default();
    narrowed.is_empty() || narrowed.contains(&open.topic.as_str()) || open.created_at_ms > earliest
}

fn options(req: &PackRequest, found: &[Block<'_>], style: &Style<'_>) -> Vec<Picks> {
    found
        .iter()
        .enumerate()
        .map(|(index, block)| {
            let known = req
                .topics
                .iter()
                .filter(|open| offered(req, block, open))
                .map(|open| Pick {
                    choice: Choice::new(None, Some(open.topic.clone()), open.topic.clone()),
                    text: topic_label(req, open, style),
                });
            let earlier = found
                .iter()
                .take(index)
                .enumerate()
                .map(|(before, other)| Pick {
                    choice: Choice::new(Some(before), None, alias("S", before)),
                    text: label_of(req, other, style),
                });
            let (value, text) = if block.auto() {
                (NONE, &req.none_label)
            } else {
                (NEW, &req.new_label)
            };
            let fresh = Pick {
                choice: Choice::new(None, None, value.to_owned()),
                text: text.clone(),
            };
            let all: Picks = known.chain(earlier).chain([fresh]).collect();
            if block.attach.is_some() || !block.linked.is_empty() || all.len() < 2 {
                Vec::new()
            } else {
                all
            }
        })
        .collect()
}

pub(crate) fn pack(req: &PackRequest, seal: Option<&Seal>) -> Result<PackResponse, AppError> {
    let mask = Mask::new(&req.rules, seal)?;
    let max = usize::try_from(req.max_chars).unwrap_or(usize::MAX);
    let rows: Vec<&Row> = req.lines.iter().map(|entry| &entry.row).collect();
    let style = Style::new(&mask, max).with_copies(&rows);
    let found = blocks(req);
    let drafted = draft(req, &found, &style);
    let options = options(req, &found, &style);
    let (shown, checks) = reviews(req, &style, max);
    let mut next = drafted.urgent.len().saturating_add(checks.len());
    let segments: Vec<Segment> = found
        .iter()
        .zip(&options)
        .map(|(block, picks)| {
            let question = (!picks.is_empty()).then(|| {
                let slot = next;
                next = next.saturating_add(1);
                slot
            });
            Segment::new(
                block.attach.map(str::to_owned),
                picks.iter().map(|pick| pick.choice.clone()).collect(),
                label_of(req, block, &style),
            )
            .with_linked(
                block
                    .linked
                    .iter()
                    .map(|topic| (*topic).to_owned())
                    .collect(),
            )
            .with_question(question)
            .with_role(if block.auto() { AUTO } else { "" })
        })
        .collect();
    let asked = options
        .iter()
        .enumerate()
        .filter(|&(_, picks)| !picks.is_empty())
        .map(|(index, picks)| choice(&req.questions.topic, index, picks));
    let questions: Vec<Value> = drafted
        .urgent
        .into_iter()
        .chain(checks)
        .chain(asked)
        .collect();
    let count = questions.len();
    let input: Vec<Value> = drafted.input.into_iter().chain(shown).collect();
    let body = json!({
        "input": Value::Array(input).to_string(),
        "model": req.model,
        "questions": questions,
    });
    Ok(PackResponse {
        body: to_string(&body).unwrap_or_default(),
        layout: Layout::new(req.batch_id, drafted.lines, segments)
            .with_reviews(
                req.reviews
                    .iter()
                    .map(|review| review.tag.clone())
                    .collect(),
            )
            .with_probabilities(history(req)),
        questions: count,
    })
}

fn predicate(question: &Question, number: usize) -> Value {
    json!({
        "instructions": question.instructions.replace(LINE, &alias("L", number)),
        "name": format!("{}_{}", question.name, number.saturating_add(1)),
        "type": "predicate",
    })
}

fn reviews(req: &PackRequest, style: &Style<'_>, max: usize) -> Checks {
    let question = &req.questions.review;
    req.reviews
        .iter()
        .enumerate()
        .map(|(index, review)| {
            let name = alias("R", index);
            let side =
                |text: &str| -> String { style.mask().apply(text).chars().take(max).collect() };
            (
                json!({"left": side(&review.left), "review": name, "right": side(&review.right)}),
                json!({
                    "instructions": question.instructions.replace(REVIEW, &name),
                    "name": format!("{}_{}", question.name, index.saturating_add(1)),
                    "type": "predicate",
                }),
            )
        })
        .unzip()
}

fn topic_label(req: &PackRequest, open: &Open, style: &Style<'_>) -> String {
    let chars = usize::try_from(req.label_chars).unwrap_or(usize::MAX);
    open.first.as_deref().map_or_else(
        || style.mask().apply(&open.label),
        |first| style.mask().apply(first).chars().take(chars).collect(),
    )
}
