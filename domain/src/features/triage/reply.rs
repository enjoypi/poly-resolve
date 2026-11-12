use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, from_str};

use crate::AppError;

const AGENT_MESSAGE: &str = "agent_message";
const ITEM_COMPLETED: &str = "item.completed";
const STATES: [&str; 3] = ["cancelled", "open", "resolved"];
const THREAD_STARTED: &str = "thread.started";

#[derive(Deserialize)]
struct Event {
    item: Option<Item>,
    #[serde(rename = "type")]
    kind: String,
    thread_id: Option<String>,
}

#[derive(Deserialize)]
struct Header {
    #[serde(default)]
    changed: Value,
    #[serde(default)]
    changes: String,
    status: String,
    #[serde(default)]
    title: String,
}

#[derive(Deserialize)]
struct Item {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

#[derive(Clone, Copy, JsonSchema, Serialize)]
#[schemars(rename = "TriageOutcome")]
#[serde(rename_all = "lowercase")]
pub(crate) enum Outcome {
    Notify,
    Quiet,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageReplyRequest")]
pub(crate) struct ReplyRequest {
    max_chars: u32,
    #[serde(default)]
    state: String,
    stdout: String,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriageReplyResponse")]
pub(crate) struct ReplyResponse {
    changes: String,
    outcome: Outcome,
    status: String,
    text: String,
    thread_id: String,
    title: String,
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
    let answer = events
        .iter()
        .rev()
        .filter(|event| event.kind == ITEM_COMPLETED)
        .filter_map(|event| event.item.as_ref())
        .find(|item| item.kind == AGENT_MESSAGE)
        .and_then(|item| item.text.as_deref())
        .map(str::trim)
        .ok_or(AppError::BadRequest)?;
    let (head, rest) = answer.split_once('\n').ok_or(AppError::BadRequest)?;
    let header: Header = from_str(head).map_err(|_err| AppError::BadRequest)?;
    let text = rest.trim();
    if !STATES.contains(&header.status.as_str()) {
        return Err(AppError::BadRequest);
    }
    let max = usize::try_from(req.max_chars).unwrap_or(usize::MAX);
    let settled = header.changed == Value::Bool(false) && header.status == req.state;
    Ok(ReplyResponse {
        changes: header.changes.chars().take(max).collect(),
        outcome: if settled {
            Outcome::Quiet
        } else {
            Outcome::Notify
        },
        status: header.status,
        text: text.chars().take(max).collect(),
        thread_id,
        title: header.title.chars().take(max).collect(),
    })
}
