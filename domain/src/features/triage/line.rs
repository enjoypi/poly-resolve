use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::redact::{Mask, Rule};
use crate::AppError;
use crate::features::ingest::cipher::sealed;

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageLineRequest")]
pub(crate) struct LineRequest {
    cipher_min_length: Option<u32>,
    max_chars: u32,
    message: Subject,
    notify_conversation_id: Option<String>,
    rules: Vec<Rule>,
    self_sender_id: Option<String>,
    unknown_sender: String,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriageLineResponse")]
pub(crate) struct LineResponse {
    sender: String,
    skip: Option<Skip>,
    text: String,
}

#[derive(Clone, Copy, JsonSchema, Serialize)]
#[schemars(rename = "TriageSkip")]
#[serde(rename_all = "lowercase")]
pub(crate) enum Skip {
    Empty,
    Notice,
    Own,
    Sealed,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageSubject")]
pub(crate) struct Subject {
    conversation_id: String,
    sender_id: String,
    sender_name: String,
    text: String,
}

pub(crate) fn line(req: &LineRequest) -> Result<LineResponse, AppError> {
    if let Some(skip) = reason(req) {
        return Ok(LineResponse {
            sender: String::new(),
            skip: Some(skip),
            text: String::new(),
        });
    }
    let mask = Mask::new(&req.rules)?;
    let message = &req.message;
    let name = if message.sender_name.is_empty() {
        &req.unknown_sender
    } else {
        &message.sender_name
    };
    let max = usize::try_from(req.max_chars).unwrap_or(usize::MAX);
    Ok(LineResponse {
        sender: mask.apply(name),
        skip: None,
        text: mask.apply(&message.text).chars().take(max).collect(),
    })
}

fn reason(req: &LineRequest) -> Option<Skip> {
    let message = &req.message;
    if req.self_sender_id.as_deref() == Some(message.sender_id.as_str()) {
        Some(Skip::Own)
    } else if req.notify_conversation_id.as_deref() == Some(message.conversation_id.as_str()) {
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
