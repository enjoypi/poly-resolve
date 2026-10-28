use chrono::{DateTime, NaiveDateTime, ParseError};
use serde_json::Value;

use super::bad;
use crate::AppError;

const LOCAL_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

pub(super) enum Stamp {
    Millis(u64),
    Text(String),
}

impl Stamp {
    pub(super) fn from_value(value: &Value) -> Option<Self> {
        value
            .as_u64()
            .map(Self::Millis)
            .or_else(|| value.as_str().map(|text| Self::Text(text.to_owned())))
    }

    pub(super) fn millis(self, offset_minutes: i16) -> Result<u64, AppError> {
        match self {
            Self::Millis(millis) => Ok(millis),
            Self::Text(text) => text_millis(&text, offset_minutes),
        }
    }
}

fn local(text: &str, offset_minutes: i16) -> Result<i64, ParseError> {
    let naive = NaiveDateTime::parse_from_str(text, LOCAL_FORMAT)?;
    Ok(naive
        .and_utc()
        .timestamp_millis()
        .saturating_sub(offset_millis(offset_minutes)))
}

pub(super) fn offset_millis(offset_minutes: i16) -> i64 {
    i64::from(offset_minutes).saturating_mul(60_000)
}

fn text_millis(text: &str, offset_minutes: i16) -> Result<u64, AppError> {
    let millis = DateTime::parse_from_rfc3339(text)
        .map(|stamp| stamp.timestamp_millis())
        .or_else(|_err| local(text, offset_minutes))
        .map_err(bad)?;
    Ok(u64::try_from(millis).unwrap_or_default())
}
