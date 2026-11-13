use core::fmt::Debug;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::AppError;

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[schemars(rename = "UsageDay")]
pub struct Day {
    calls: u64,
    date: String,
    failures: u64,
    input_tokens: u64,
    kind: String,
    output_tokens: u64,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, Serialize)]
#[schemars(rename = "SourcesHealth")]
pub struct Health {
    sources: Vec<Source>,
    stages: Vec<Stage>,
}

pub trait Meter: Debug + Sync {
    fn sources(&self) -> Result<Health, AppError>;
    fn usage(&self, days: u32) -> Result<Vec<Day>, AppError>;
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[schemars(rename = "SourcesItem")]
pub struct Source {
    account: String,
    down_at_ms: u64,
    kind: String,
    last_at_ms: Option<u64>,
    ok_at_ms: u64,
    state: String,
    streak: u64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[schemars(rename = "SourcesStage")]
pub struct Stage {
    kind: String,
    streak: u64,
}

#[derive(Debug)]
pub(crate) struct Still;

impl Meter for Still {
    fn sources(&self) -> Result<Health, AppError> {
        Ok(Health::default())
    }

    fn usage(&self, _days: u32) -> Result<Vec<Day>, AppError> {
        Ok(Vec::new())
    }
}
