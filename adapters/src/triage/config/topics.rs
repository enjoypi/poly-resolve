use alloc::collections::BTreeMap;
use core::num::{NonZeroU32, NonZeroU64};

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Clue {
    kind: String,
    pattern: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Clues {
    max_conversations: NonZeroU32,
    patterns: Vec<Clue>,
    subject: Subject,
}

impl Clues {
    pub(crate) const fn max_conversations(&self) -> u32 {
        self.max_conversations.get()
    }

    pub(crate) fn patterns(&self) -> &[Clue] {
        &self.patterns
    }

    pub(crate) const fn subject(&self) -> &Subject {
        &self.subject
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Notice {
    late: String,
    late_after_ms: u64,
    origins: BTreeMap<String, String>,
    states: BTreeMap<String, String>,
    summary: Summary,
    template: String,
}

impl Notice {
    pub(crate) fn late(&self) -> &str {
        &self.late
    }

    pub(crate) const fn late_after_ms(&self) -> u64 {
        self.late_after_ms
    }

    pub(crate) const fn origins(&self) -> &BTreeMap<String, String> {
        &self.origins
    }

    pub(crate) const fn states(&self) -> &BTreeMap<String, String> {
        &self.states
    }

    pub(crate) const fn summary(&self) -> &Summary {
        &self.summary
    }

    pub(crate) fn template(&self) -> &str {
        &self.template
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Reach {
    cap: f64,
    conversations: f64,
    people: f64,
    sources: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Subject {
    ignore: Vec<String>,
    min_chars: u32,
    prefixes: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Summary {
    item: String,
    template: String,
}

impl Summary {
    pub(crate) fn item(&self) -> &str {
        &self.item
    }

    pub(crate) fn template(&self) -> &str {
        &self.template
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Topics {
    archive_after_ms: NonZeroU64,
    clues: Clues,
    max_open: NonZeroU32,
    merge_after: NonZeroU32,
    reach: Reach,
    review_max: u32,
    split_after: NonZeroU32,
}

impl Topics {
    pub(crate) const fn archive_after_ms(&self) -> u64 {
        self.archive_after_ms.get()
    }

    pub(crate) const fn clues(&self) -> &Clues {
        &self.clues
    }

    pub(crate) const fn max_open(&self) -> u32 {
        self.max_open.get()
    }

    pub(crate) const fn merge_after(&self) -> u32 {
        self.merge_after.get()
    }

    pub(crate) const fn reach(&self) -> &Reach {
        &self.reach
    }

    pub(crate) const fn review_max(&self) -> u32 {
        self.review_max
    }

    pub(crate) const fn split_after(&self) -> u32 {
        self.split_after.get()
    }
}
