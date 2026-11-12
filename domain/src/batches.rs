use core::fmt::Debug;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::AppError;
use crate::outbound::Listing;

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[schemars(rename = "BatchItem")]
pub struct Item {
    attempts: u64,
    batch_id: u64,
    lines: u64,
    reason: String,
    state: String,
    updated_at_ms: u64,
}

pub trait Roster: Debug + Sync {
    fn list(&self, listing: &Listing<'_>) -> Result<Vec<Item>, AppError>;
    fn revive(&self, batch_id: u64) -> Result<Option<String>, AppError>;
}
