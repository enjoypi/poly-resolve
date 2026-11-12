use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::redact::{Mask, Rule};
use crate::AppError;
use crate::features::ingest::cipher::sealed;
use crate::redact::Seal;

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageLineRequest")]
pub(crate) struct LineRequest {
    cipher_min_length: Option<u32>,
    #[serde(default)]
    digest_min_chars: Option<u32>,
    max_chars: u32,
    message: Subject,
    notify_conversation_id: Option<String>,
    rules: Vec<Rule>,
    self_sender_ids: Vec<String>,
    unknown_sender: String,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriageLineResponse")]
pub(crate) struct LineResponse {
    digest: String,
    role: Option<Role>,
    sender: String,
    skip: Option<Skip>,
    state: State,
    text: String,
}

#[derive(Clone, Copy, JsonSchema, Serialize)]
#[schemars(rename = "TriageRole")]
#[serde(rename_all = "lowercase")]
pub(crate) enum Role {
    Auto,
    Own,
}

#[derive(Clone, Copy, JsonSchema, Serialize)]
#[schemars(rename = "TriageSkip")]
#[serde(rename_all = "lowercase")]
pub(crate) enum Skip {
    Empty,
    Notice,
    Sealed,
}

impl Skip {
    const fn state(self) -> State {
        match self {
            Self::Notice => State::Skipped,
            Self::Empty | Self::Sealed => State::Withheld,
        }
    }
}

#[derive(Clone, Copy, JsonSchema, Serialize)]
#[schemars(rename = "TriageLineState")]
#[serde(rename_all = "lowercase")]
pub(crate) enum State {
    Queued,
    Skipped,
    Withheld,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageSubject")]
pub(crate) struct Subject {
    #[serde(default)]
    bulk: Option<String>,
    conversation_id: String,
    sender_id: String,
    sender_name: String,
    text: String,
}

fn digest(req: &LineRequest, seal: Option<&Seal>) -> String {
    let text = &req.message.text;
    req.digest_min_chars
        .and_then(|min| usize::try_from(min).ok())
        .filter(|min| text.chars().count() >= *min)
        .and(seal)
        .map_or_default(|found| found.digest(text))
}

pub(crate) fn line(req: &LineRequest, seal: Option<&Seal>) -> Result<LineResponse, AppError> {
    let role = role(req);
    if let Some(skip) = reason(req) {
        return Ok(LineResponse {
            digest: String::new(),
            role,
            sender: String::new(),
            skip: Some(skip),
            state: skip.state(),
            text: String::new(),
        });
    }
    let mask = Mask::new(&req.rules, seal)?;
    let message = &req.message;
    let name = if message.sender_name.is_empty() {
        &req.unknown_sender
    } else {
        &message.sender_name
    };
    let max = usize::try_from(req.max_chars).unwrap_or(usize::MAX);
    Ok(LineResponse {
        digest: digest(req, seal),
        role,
        sender: mask.apply(name),
        skip: None,
        state: State::Queued,
        text: mask.apply(&message.text).chars().take(max).collect(),
    })
}

fn reason(req: &LineRequest) -> Option<Skip> {
    let message = &req.message;
    if req.notify_conversation_id.as_deref() == Some(message.conversation_id.as_str()) {
        Some(Skip::Notice)
    } else if message.text.trim().is_empty() {
        Some(Skip::Empty)
    } else if req
        .cipher_min_length
        .is_some_and(|min| sealed(&message.text, usize::try_from(min).unwrap_or(usize::MAX)))
    {
        Some(Skip::Sealed)
    } else {
        None
    }
}

fn role(req: &LineRequest) -> Option<Role> {
    let message = &req.message;
    if req
        .self_sender_ids
        .iter()
        .any(|own| own.eq_ignore_ascii_case(&message.sender_id))
    {
        Some(Role::Own)
    } else {
        message.bulk.as_ref().map(|_| Role::Auto)
    }
}
