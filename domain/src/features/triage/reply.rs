use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::from_str;

use crate::AppError;

const AGENT_MESSAGE: &str = "agent_message";
const ITEM_COMPLETED: &str = "item.completed";
const THREAD_STARTED: &str = "thread.started";

#[derive(Deserialize)]
struct Event {
    item: Option<Item>,
    #[serde(rename = "type")]
    kind: String,
    thread_id: Option<String>,
}

#[derive(Deserialize)]
struct Item {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageReplyRequest")]
pub(crate) struct ReplyRequest {
    max_chars: u32,
    stdout: String,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriageReplyResponse")]
pub(crate) struct ReplyResponse {
    text: String,
    thread_id: String,
}

pub(crate) fn reply(req: &ReplyRequest) -> Result<ReplyResponse, AppError> {
    let events: Vec<Event> = req
        .stdout
        .lines()
        .filter_map(|line| from_str(line).ok())
        .collect();
    let thread_id = events
        .iter()
        .find(|event| event.kind == THREAD_STARTED)
        .and_then(|event| event.thread_id.clone())
        .ok_or(AppError::BadRequest)?;
    let text = events
        .iter()
        .rev()
        .filter(|event| event.kind == ITEM_COMPLETED)
        .filter_map(|event| event.item.as_ref())
        .find(|item| item.kind == AGENT_MESSAGE)
        .and_then(|item| item.text.as_deref())
        .map(str::trim)
        .filter(|body| !body.is_empty())
        .ok_or(AppError::BadRequest)?;
    let max = usize::try_from(req.max_chars).unwrap_or(usize::MAX);
    Ok(ReplyResponse {
        text: text.chars().take(max).collect(),
        thread_id,
    })
}
