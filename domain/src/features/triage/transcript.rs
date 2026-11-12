use alloc::collections::BTreeMap;

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::redact::Mask;
use crate::features::ingest::clock;

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageRow")]
pub(crate) struct Row {
    #[serde(default)]
    digest: String,
    #[serde(default)]
    key: String,
    occurred_at_ms: u64,
    #[serde(default)]
    omitted: Option<String>,
    #[serde(default)]
    role: String,
    sender: String,
    #[serde(default)]
    source: Option<String>,
    text: String,
}

impl Row {
    pub(super) fn digest(&self) -> &str {
        &self.digest
    }

    pub(super) fn key(&self) -> &str {
        &self.key
    }

    pub(super) const fn occurred_at_ms(&self) -> u64 {
        self.occurred_at_ms
    }

    pub(super) fn omitted(&self) -> Option<&str> {
        self.omitted.as_deref()
    }

    pub(super) fn render(&self, style: &Style<'_>) -> Value {
        let mut shown = json!({
            "sender": style.mask.apply(&self.sender),
            "text": style.mask.apply(&self.text).chars().take(style.max).collect::<String>(),
            "time_utc": clock(self.occurred_at_ms, 0).unwrap_or_default(),
        });
        if let Some(map) = shown.as_object_mut().filter(|_| !self.role.is_empty()) {
            drop(map.insert("role".to_owned(), Value::String(self.role.clone())));
        }
        if let Some((map, source)) = shown.as_object_mut().zip(self.source.as_ref()) {
            drop(map.insert("source".to_owned(), Value::String(source.clone())));
        }
        let repeated = style
            .copies
            .get(&self.digest)
            .copied()
            .filter(|count| *count > 1);
        if let Some((map, count)) = shown.as_object_mut().zip(repeated) {
            drop(map.insert("copies".to_owned(), json!(count)));
        }
        shown
    }

    pub(super) fn role(&self) -> &str {
        &self.role
    }

    pub(super) fn text(&self) -> &str {
        &self.text
    }
}

pub(super) struct Style<'style> {
    copies: BTreeMap<String, usize>,
    mask: &'style Mask,
    max: usize,
}

impl<'style> Style<'style> {
    pub(super) const fn mask(&self) -> &Mask {
        self.mask
    }

    pub(super) const fn new(mask: &'style Mask, max: usize) -> Self {
        Self {
            copies: BTreeMap::new(),
            mask,
            max,
        }
    }

    pub(super) fn with_copies(self, rows: &[&Row]) -> Self {
        let mut copies: BTreeMap<String, usize> = BTreeMap::new();
        for row in rows
            .iter()
            .filter(|row| row.omitted.is_none() && !row.digest.is_empty())
        {
            let count = copies.entry(row.digest.clone()).or_default();
            *count = count.saturating_add(1);
        }
        Self { copies, ..self }
    }
}
