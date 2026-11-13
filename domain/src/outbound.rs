use core::fmt::Debug;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::AppError;

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[schemars(rename = "OutboundItem")]
pub struct Item {
    account: String,
    batch_id: u64,
    conversation_id: String,
    flag: String,
    key: String,
    occurred_at_ms: u64,
    reason: String,
    role: String,
    sender: String,
    source: String,
    state: String,
    text: String,
    #[serde(default)]
    version: String,
}

#[derive(Clone, Copy, Debug)]
pub struct Listing<'listing> {
    limit: u32,
    state: &'listing str,
}

impl<'listing> Listing<'listing> {
    #[must_use = "条数需交给查询"]
    pub const fn limit(&self) -> u32 {
        self.limit
    }

    #[must_use = "查询需交给存储"]
    pub const fn new(state: &'listing str, limit: u32) -> Self {
        Self { limit, state }
    }

    #[must_use = "状态需交给查询"]
    pub const fn state(&self) -> &str {
        self.state
    }
}

pub trait Outbox: Debug + Sync {
    fn list(&self, listing: &Listing<'_>) -> Result<Vec<Item>, AppError>;
}
