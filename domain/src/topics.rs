use core::fmt::Debug;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::AppError;
use crate::outbound::Listing;

#[derive(Debug)]
pub(crate) struct Blank;

impl Board for Blank {
    fn detail(&self, _topic_id: &str) -> Result<Option<Detail>, AppError> {
        Ok(None)
    }

    fn overview(&self, _listing: &Listing<'_>) -> Result<Overview, AppError> {
        Ok(Overview::default())
    }
}

pub trait Board: Debug + Sync {
    fn detail(&self, topic_id: &str) -> Result<Option<Detail>, AppError>;
    fn overview(&self, listing: &Listing<'_>) -> Result<Overview, AppError>;
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[schemars(rename = "TopicChange")]
pub struct Change {
    at_ms: u64,
    from_topic: String,
    keys: u64,
    kind: String,
    reason: String,
    serial: u64,
    state: String,
    to_topic: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[schemars(rename = "TopicDetail")]
pub struct Detail {
    changes: Vec<Change>,
    timeline: Vec<Entry>,
    topic: Summary,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[schemars(rename = "TopicEntry")]
pub struct Entry {
    conversation_id: String,
    key: String,
    occurred_at_ms: u64,
    #[serde(default)]
    purged_at_ms: u64,
    reason: String,
    role: String,
    sender: String,
    source: String,
    state: String,
    text: String,
    via: String,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[schemars(rename = "TopicOverview")]
pub struct Overview {
    items: Vec<Summary>,
    lines: Vec<Tally>,
    topics: Vec<Tally>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[schemars(rename = "TopicItem")]
pub struct Summary {
    conversations: u64,
    first_at_ms: u64,
    label: String,
    last_at_ms: u64,
    members: u64,
    people: u64,
    previous: Option<f64>,
    score: Option<f64>,
    sources: Vec<String>,
    state: String,
    topic_id: String,
    urgency: Option<f64>,
    verdict: String,
    #[serde(default)]
    version: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[schemars(rename = "TopicTally")]
pub struct Tally {
    count: u64,
    reason: String,
    state: String,
}
