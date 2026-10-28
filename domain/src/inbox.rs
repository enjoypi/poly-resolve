use core::fmt::Debug;
use core::num::NonZeroU32;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::AppError;

#[derive(Clone, Debug)]
pub struct Mark {
    key: String,
    occurred_at_ms: u64,
}

impl Mark {
    #[must_use = "游标位置需交给查询"]
    pub fn key(&self) -> &str {
        &self.key
    }

    #[must_use = "游标位置需交给查询"]
    pub const fn new(key: String, occurred_at_ms: u64) -> Self {
        Self {
            key,
            occurred_at_ms,
        }
    }

    #[must_use = "游标位置需交给查询"]
    pub const fn occurred_at_ms(&self) -> u64 {
        self.occurred_at_ms
    }

    pub(crate) fn parse(cursor: &str) -> Result<Self, AppError> {
        let (millis, key) = cursor.split_once(':').ok_or(AppError::BadRequest)?;
        let occurred_at_ms = millis.parse().map_err(|_err| AppError::BadRequest)?;
        Ok(Self::new(key.to_owned(), occurred_at_ms))
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Paging {
    default: NonZeroU32,
    max: NonZeroU32,
}

impl Paging {
    #[must_use = "条数需交给查询"]
    pub const fn default_limit(self) -> u32 {
        self.default.get()
    }

    #[must_use = "条数需交给查询"]
    pub const fn max_limit(self) -> u32 {
        self.max.get()
    }
}

#[derive(Debug)]
pub struct Query {
    cursor: Option<Mark>,
    limit: u32,
}

impl Query {
    #[must_use = "游标需交给查询"]
    pub const fn cursor(&self) -> Option<&Mark> {
        self.cursor.as_ref()
    }

    #[must_use = "条数需交给查询"]
    pub const fn limit(&self) -> u32 {
        self.limit
    }

    #[must_use = "查询需交给读取方"]
    pub const fn new(cursor: Option<Mark>, limit: u32) -> Self {
        Self { cursor, limit }
    }
}

pub trait Reader: Debug + Sync {
    fn page(&self, query: &Query) -> Result<Vec<Row>, AppError>;
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[schemars(rename = "MessagesListItem")]
pub struct Row {
    account: String,
    conversation_id: String,
    key: String,
    occurred_at_ms: u64,
    received_at_ms: u64,
    sender_id: String,
    sender_name: String,
    source: String,
    text: String,
}

impl Row {
    pub(crate) fn cursor(&self) -> String {
        format!("{}:{}", self.occurred_at_ms, self.key)
    }

    #[must_use = "键需用于排序与游标"]
    pub fn key(&self) -> &str {
        &self.key
    }

    #[must_use = "时间需用于排序与游标"]
    pub const fn occurred_at_ms(&self) -> u64 {
        self.occurred_at_ms
    }
}
