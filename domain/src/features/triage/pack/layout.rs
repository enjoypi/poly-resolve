use alloc::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Deserialize, JsonSchema, Serialize)]
#[schemars(rename = "TriageChoice")]
pub(crate) struct Choice {
    segment: Option<usize>,
    topic: Option<String>,
    value: String,
}

impl Choice {
    pub(crate) fn find<'choice>(choices: &'choice [Self], value: &str) -> Option<&'choice Self> {
        choices.iter().find(|choice| choice.value == value)
    }

    pub(crate) const fn new(segment: Option<usize>, topic: Option<String>, value: String) -> Self {
        Self {
            segment,
            topic,
            value,
        }
    }

    pub(crate) const fn segment(&self) -> Option<usize> {
        self.segment
    }

    pub(crate) fn topic(&self) -> Option<&str> {
        self.topic.as_deref()
    }

    pub(crate) fn value(&self) -> &str {
        &self.value
    }
}

#[derive(Deserialize, JsonSchema, Serialize)]
#[schemars(rename = "TriageLayout")]
pub(crate) struct Layout {
    batch_id: u64,
    lines: Vec<Placed>,
    #[serde(default)]
    probabilities: BTreeMap<String, Option<f64>>,
    #[serde(default)]
    reviews: Vec<Value>,
    segments: Vec<Segment>,
}

impl Layout {
    pub(crate) const fn batch_id(&self) -> u64 {
        self.batch_id
    }

    pub(crate) fn lines(&self) -> &[Placed] {
        &self.lines
    }

    pub(crate) const fn new(batch_id: u64, lines: Vec<Placed>, segments: Vec<Segment>) -> Self {
        Self {
            batch_id,
            lines,
            probabilities: BTreeMap::new(),
            reviews: Vec::new(),
            segments,
        }
    }

    pub(crate) fn predicates(&self) -> usize {
        self.lines
            .iter()
            .filter(|line| line.role.is_empty())
            .count()
    }

    pub(crate) const fn probabilities(&self) -> &BTreeMap<String, Option<f64>> {
        &self.probabilities
    }

    pub(crate) fn questions(&self) -> usize {
        self.scored().saturating_add(
            self.segments
                .iter()
                .filter(|segment| segment.question.is_some())
                .count(),
        )
    }

    pub(crate) fn reviews(&self) -> &[Value] {
        &self.reviews
    }

    pub(crate) fn scored(&self) -> usize {
        self.predicates().saturating_add(self.reviews.len())
    }

    pub(crate) fn segments(&self) -> &[Segment] {
        &self.segments
    }

    pub(crate) fn with_probabilities(self, probabilities: BTreeMap<String, Option<f64>>) -> Self {
        Self {
            probabilities,
            ..self
        }
    }

    pub(crate) fn with_reviews(self, reviews: Vec<Value>) -> Self {
        Self { reviews, ..self }
    }
}

#[derive(Deserialize, JsonSchema, Serialize)]
#[schemars(rename = "TriagePlaced")]
pub(crate) struct Placed {
    key: String,
    occurred_at_ms: u64,
    #[serde(default)]
    role: String,
    segment: usize,
}

impl Placed {
    pub(crate) fn key(&self) -> &str {
        &self.key
    }

    pub(crate) const fn new(key: String, occurred_at_ms: u64, segment: usize) -> Self {
        Self {
            key,
            occurred_at_ms,
            role: String::new(),
            segment,
        }
    }

    pub(crate) const fn occurred_at_ms(&self) -> u64 {
        self.occurred_at_ms
    }

    pub(crate) fn role(&self) -> &str {
        &self.role
    }

    pub(crate) const fn segment(&self) -> usize {
        self.segment
    }

    pub(crate) fn with_role(self, role: &str) -> Self {
        Self {
            role: role.to_owned(),
            ..self
        }
    }
}

#[derive(Deserialize, JsonSchema, Serialize)]
#[schemars(rename = "TriageSegment")]
pub(crate) struct Segment {
    attach: Option<String>,
    choices: Vec<Choice>,
    label: String,
    #[serde(default)]
    linked: Vec<String>,
    question: Option<usize>,
    #[serde(default)]
    role: String,
}

impl Segment {
    pub(crate) fn attach(&self) -> Option<&str> {
        self.attach.as_deref()
    }

    pub(crate) fn choices(&self) -> &[Choice] {
        &self.choices
    }

    pub(crate) fn label(&self) -> &str {
        &self.label
    }

    pub(crate) fn linked(&self) -> &[String] {
        &self.linked
    }

    pub(crate) const fn new(attach: Option<String>, choices: Vec<Choice>, label: String) -> Self {
        Self {
            attach,
            choices,
            label,
            linked: Vec::new(),
            question: None,
            role: String::new(),
        }
    }

    pub(crate) const fn question(&self) -> Option<usize> {
        self.question
    }

    pub(crate) fn role(&self) -> &str {
        &self.role
    }

    pub(crate) fn with_linked(self, linked: Vec<String>) -> Self {
        Self { linked, ..self }
    }

    pub(crate) fn with_question(self, question: Option<usize>) -> Self {
        Self { question, ..self }
    }

    pub(crate) fn with_role(self, role: &str) -> Self {
        Self {
            role: role.to_owned(),
            ..self
        }
    }
}
