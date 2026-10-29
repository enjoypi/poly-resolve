use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::redact::Mask;
use crate::features::ingest::clock;

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageRow")]
pub(crate) struct Row {
    occurred_at_ms: u64,
    sender: String,
    text: String,
}

impl Row {
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

pub(super) fn fit(mut lines: Vec<Value>, max: usize) -> String {
    loop {
        let text = Value::Array(lines.clone()).to_string();
        if text.chars().count() <= max || lines.len() <= 1 {
            return text;
        }
        drop(lines.remove(0));
    }
}
