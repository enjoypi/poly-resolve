use core::fmt::Debug;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::AppError;

const PENDING: &str = "pending";
const SCREENING: &str = "screening";

pub trait Docket: Debug + Sync {
    fn draw(&self, pick: Pick) -> Result<Option<Drawn>, AppError>;
    fn list(&self, listing: &Listing<'_>) -> Result<Vec<Item>, AppError>;
    fn settle(&self, ruling: &Ruling<'_>) -> Result<Option<String>, AppError>;
    fn source(&self, key: &str) -> Result<Option<String>, AppError>;
}

#[derive(Clone, Copy, Debug)]
pub struct Draw {
    limit: u32,
    state: Option<&'static str>,
}

impl Draw {
    #[must_use = "策略需交给 Rules"]
    pub const fn off() -> Self {
        Self {
            limit: 0,
            state: None,
        }
    }

    #[must_use = "取批参数需交给审核台"]
    pub const fn pick(self) -> Option<Pick> {
        match self.state {
            Some(state) => Some(Pick {
                limit: self.limit,
                state,
            }),
            None => None,
        }
    }

    #[must_use = "策略需交给 Rules"]
    pub const fn reviewed(limit: u32) -> Self {
        Self {
            limit,
            state: Some(PENDING),
        }
    }

    #[must_use = "策略需交给 Rules"]
    pub const fn screened(limit: u32) -> Self {
        Self {
            limit,
            state: Some(SCREENING),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Drawn {
    batch_id: u64,
    lines: u64,
}

impl Drawn {
    #[must_use = "批号需写回响应"]
    pub const fn batch_id(self) -> u64 {
        self.batch_id
    }

    #[must_use = "条数需写回响应"]
    pub const fn lines(self) -> u64 {
        self.lines
    }

    #[must_use = "结果需交给 review.draw"]
    pub const fn new(batch_id: u64, lines: u64) -> Self {
        Self { batch_id, lines }
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[schemars(rename = "ReviewItem")]
pub struct Item {
    account: String,
    batch_id: u64,
    conversation_id: String,
    flag: String,
    key: String,
    occurred_at_ms: u64,
    reason: String,
    sender: String,
    source: String,
    state: String,
    text: String,
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

    #[must_use = "查询需交给审核台"]
    pub const fn new(state: &'listing str, limit: u32) -> Self {
        Self { limit, state }
    }

    #[must_use = "状态需交给查询"]
    pub const fn state(&self) -> &str {
        self.state
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Pick {
    limit: u32,
    state: &'static str,
}

impl Pick {
    #[must_use = "条数需交给查询"]
    pub const fn limit(self) -> u32 {
        self.limit
    }

    #[must_use = "状态需写入审核项"]
    pub const fn state(self) -> &'static str {
        self.state
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[schemars(rename = "ReviewReason")]
#[serde(deny_unknown_fields)]
pub struct Reason {
    code: String,
    label: String,
    #[serde(default)]
    placeholder: Option<String>,
}

impl Reason {
    pub(crate) fn code(&self) -> &str {
        &self.code
    }

    #[must_use = "占位符需交给标红"]
    pub fn placeholder(&self) -> Option<&str> {
        self.placeholder.as_deref()
    }
}

#[derive(Clone, Debug)]
pub struct Rule {
    pattern: String,
    placeholder: String,
}

impl Rule {
    #[must_use = "规则需交给裁决"]
    pub const fn new(pattern: String, placeholder: String) -> Self {
        Self {
            pattern,
            placeholder,
        }
    }

    #[must_use = "模式需写入规则表"]
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    #[must_use = "占位符需写入规则表"]
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Ruling<'ruling> {
    key: &'ruling str,
    reason: Option<&'ruling str>,
    rule: Option<&'ruling Rule>,
}

impl<'ruling> Ruling<'ruling> {
    #[must_use = "裁决需交给审核台"]
    pub const fn approve(key: &'ruling str) -> Self {
        Self {
            key,
            reason: None,
            rule: None,
        }
    }

    #[must_use = "键需用于定位审核项"]
    pub const fn key(&self) -> &str {
        self.key
    }

    #[must_use = "裁决需交给审核台"]
    pub const fn learned(self, rule: &'ruling Rule) -> Self {
        Self {
            rule: Some(rule),
            ..self
        }
    }

    #[must_use = "原因需写入审核项"]
    pub fn reason(&self) -> &str {
        self.reason.unwrap_or_default()
    }

    #[must_use = "裁决需交给审核台"]
    pub const fn reject(key: &'ruling str, reason: &'ruling str) -> Self {
        Self {
            key,
            reason: Some(reason),
            rule: None,
        }
    }

    #[must_use = "规则需写入规则表"]
    pub const fn rule(&self) -> Option<&Rule> {
        self.rule
    }

    #[must_use = "状态需写入审核项"]
    pub fn state(&self) -> &'static str {
        if self.rule.is_some() {
            "leaked"
        } else {
            self.reason.map_or("approved", |_| "rejected")
        }
    }
}
