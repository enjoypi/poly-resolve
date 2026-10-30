use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::redact::Mask;
use crate::features::ingest::clock;

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageRow")]
pub(crate) struct Row {
    #[serde(default)]
    key: String,
    occurred_at_ms: u64,
    sender: String,
    text: String,
}

impl Row {
    pub(super) fn key(&self) -> &str {
        &self.key
    }

    pub(super) const fn occurred_at_ms(&self) -> u64 {
        self.occurred_at_ms
    }

    pub(super) fn render(&self, style: &Style<'_>) -> Value {
        json!({
            "sender": style.mask.apply(&self.sender),
            "text": style.mask.apply(&self.text).chars().take(style.max).collect::<String>(),
            "time_utc": clock(self.occurred_at_ms, 0).unwrap_or_default(),
        })
    }

    pub(super) fn text(&self) -> &str {
        &self.text
    }
}

pub(super) struct Style<'style> {
    mask: &'style Mask,
    max: usize,
}

impl<'style> Style<'style> {
    pub(super) const fn mask(&self) -> &Mask {
        self.mask
    }

    pub(super) const fn new(mask: &'style Mask, max: usize) -> Self {
        Self { mask, max }
    }
}

pub(super) fn fit(lines: Vec<Value>, max: usize) -> String {
    let sizes: Vec<usize> = lines
        .iter()
        .map(|line| line.to_string().chars().count().saturating_add(1))
        .collect();
    let mut total = sizes
        .iter()
        .fold(1, |sum: usize, size| sum.saturating_add(*size));
    let skip = sizes
        .iter()
        .take(sizes.len().saturating_sub(1))
        .take_while(|size| {
            let over = total > max;
            total = total.saturating_sub(**size);
            over
        })
        .count();
    Value::Array(lines.into_iter().skip(skip).collect()).to_string()
}
