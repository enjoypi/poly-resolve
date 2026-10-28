use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::features::ingest::clock;

const ANALYSIS: &str = "{analysis}";
const CONVERSATION: &str = "{conversation}";
const MESSAGES: &str = "{messages}";
const PROBABILITY: &str = "{probability}";

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageBriefRequest")]
pub(crate) struct BriefRequest {
    max_chars: u32,
    rows: Vec<Line>,
    template: String,
    trigger_ms: u64,
    window_ms: u64,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriageBriefResponse")]
pub(crate) struct BriefResponse {
    prompt: String,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageLine")]
pub(crate) struct Line {
    occurred_at_ms: u64,
    sender_id: String,
    sender_name: String,
    text: String,
}

impl Line {
    fn render(&self) -> Value {
        let sender = if self.sender_name.is_empty() {
            &self.sender_id
        } else {
            &self.sender_name
        };
        json!({
            "sender": sender,
            "text": self.text,
            "time_utc": clock(self.occurred_at_ms, 0).unwrap_or_default(),
        })
    }
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageNoticeRequest")]
pub(crate) struct NoticeRequest {
    analysis: String,
    conversation_id: String,
    max_chars: u32,
    probability: f64,
    template: String,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriageNoticeResponse")]
pub(crate) struct NoticeResponse {
    text: String,
}

pub(crate) fn brief(req: BriefRequest) -> BriefResponse {
    let BriefRequest {
        max_chars,
        rows: all,
        template,
        trigger_ms,
        window_ms,
    } = req;
    let from = trigger_ms.saturating_sub(window_ms);
    let mut rows: Vec<Line> = all
        .into_iter()
        .filter(|line| (from..=trigger_ms).contains(&line.occurred_at_ms))
        .collect();
    rows.sort_by_key(|line| line.occurred_at_ms);
    let max = usize::try_from(max_chars).unwrap_or(usize::MAX);
    let messages = fit(rows.iter().map(Line::render).collect(), max);
    BriefResponse {
        prompt: template.replacen(MESSAGES, &messages, 1),
    }
}

fn fit(mut lines: Vec<Value>, max: usize) -> String {
    loop {
        let text = Value::Array(lines.clone()).to_string();
        if text.chars().count() <= max || lines.is_empty() {
            return text;
        }
        drop(lines.remove(0));
    }
}

pub(crate) fn notice(req: NoticeRequest) -> NoticeResponse {
    let NoticeRequest {
        analysis,
        conversation_id,
        max_chars,
        probability,
        template,
    } = req;
    let max = usize::try_from(max_chars).unwrap_or(usize::MAX);
    let excerpt: String = analysis.chars().take(max).collect();
    NoticeResponse {
        text: template
            .replace(CONVERSATION, &conversation_id)
            .replace(PROBABILITY, &format!("{probability:.2}"))
            .replacen(ANALYSIS, &excerpt, 1),
    }
}
