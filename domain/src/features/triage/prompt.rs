use alloc::collections::{BTreeMap, BTreeSet};

use chrono::DateTime;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::redact::{Mask, Rule};
use super::transcript::{Row, Style};
use crate::AppError;
use crate::redact::Seal;

const ANALYSIS: &str = "{analysis}";
const CHANGES: &str = "{changes}";
const CONVERSATIONS: &str = "{conversations}";
const COUNT: &str = "{count}";
const ITEMS: &str = "{items}";
const LABEL: &str = "{label}";
const LATE: &str = "{late}";
pub(crate) const MESSAGES: &str = "{messages}";
const ORIGIN: &str = "{origin}";
const PEOPLE: &str = "{people}";
const REASON: &str = "{reason}";
const SCORE: &str = "{score}";
const SOURCES: &str = "{sources}";
const STATE: &str = "{state}";
const TIME: &str = "{time}";
const TIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S UTC";
const TOPIC: &str = "{topic}";
const URGENCY: &str = "{urgency}";

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageBriefRequest")]
pub(crate) struct BriefRequest {
    followup: String,
    max_chars: u32,
    omitted: String,
    rows: Vec<Row>,
    rules: Vec<Rule>,
    template: String,
    #[serde(default)]
    thread: Option<String>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriageBriefResponse")]
pub(crate) struct BriefResponse {
    prompt: String,
    start: Start,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageNoticeRequest")]
pub(crate) struct NoticeRequest {
    analysis: String,
    #[serde(default)]
    changes: String,
    label: String,
    #[serde(default)]
    late: String,
    #[serde(default)]
    late_ms: Option<u64>,
    max_chars: u32,
    #[serde(default)]
    origin: String,
    #[serde(default)]
    origins: BTreeMap<String, String>,
    #[serde(default)]
    reach: Reach,
    #[serde(default)]
    score: Option<f64>,
    #[serde(default)]
    state: String,
    #[serde(default)]
    states: BTreeMap<String, String>,
    template: String,
    topic_id: String,
    #[serde(default)]
    urgency: Option<f64>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriageNoticeResponse")]
pub(crate) struct NoticeResponse {
    text: String,
}

#[derive(Default, Deserialize, JsonSchema)]
#[schemars(rename = "TriageReach")]
pub(crate) struct Reach {
    #[serde(default)]
    conversations: u64,
    #[serde(default)]
    people: u64,
    #[serde(default)]
    sources: u64,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageRecapItem")]
pub(crate) struct RecapItem {
    label: String,
    #[serde(default)]
    score: Option<f64>,
    state: String,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageRecapRequest")]
pub(crate) struct RecapRequest {
    item: String,
    items: Vec<RecapItem>,
    #[serde(default)]
    states: BTreeMap<String, String>,
    template: String,
}

#[derive(Clone, Copy, JsonSchema, Serialize)]
#[schemars(rename = "TriageStart")]
#[serde(rename_all = "lowercase")]
pub(crate) enum Start {
    Fresh,
    Resume,
}

pub(crate) fn brief(req: &BriefRequest, seal: Option<&Seal>) -> Result<BriefResponse, AppError> {
    let mask = Mask::new(&req.rules, seal)?;
    let max = usize::try_from(req.max_chars).unwrap_or(usize::MAX);
    let mut sorted: Vec<&Row> = req.rows.iter().collect();
    let style = Style::new(&mask, max).with_copies(&sorted);
    let (template, start) = req
        .thread
        .as_deref()
        .filter(|thread| !thread.is_empty())
        .map_or((&req.template, Start::Fresh), |_| {
            (&req.followup, Start::Resume)
        });
    sorted.sort_by_key(|row| (row.occurred_at_ms(), row.key()));
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    sorted.retain(|row| {
        row.omitted().is_some() || row.digest().is_empty() || seen.insert(row.digest())
    });
    let mut lines: Vec<Value> = Vec::new();
    for run in
        sorted.chunk_by(|left, right| left.omitted().is_some() && left.omitted() == right.omitted())
    {
        match run.first().and_then(|first| first.omitted()) {
            Some(reason) => lines.push(note(&req.omitted, run.len(), reason)),
            None => lines.extend(run.iter().map(|row| row.render(&style))),
        }
    }
    Ok(BriefResponse {
        prompt: template.replacen(MESSAGES, &Value::Array(lines).to_string(), 1),
        start,
    })
}

fn note(template: &str, count: usize, reason: &str) -> Value {
    json!({
        "note": template
            .replace(COUNT, &count.to_string())
            .replace(REASON, reason),
    })
}

pub(crate) fn notice(req: NoticeRequest) -> NoticeResponse {
    let NoticeRequest {
        analysis,
        changes,
        label,
        late,
        late_ms,
        max_chars,
        origin,
        origins,
        reach,
        score,
        state,
        states,
        template,
        topic_id,
        urgency,
    } = req;
    let max = usize::try_from(max_chars).unwrap_or(usize::MAX);
    let excerpt: String = analysis.chars().take(max).collect();
    let delayed = late_ms
        .and_then(|millis| i64::try_from(millis).ok())
        .and_then(DateTime::from_timestamp_millis)
        .map_or_default(|stamp| late.replace(TIME, &stamp.format(TIME_FORMAT).to_string()));
    NoticeResponse {
        text: template
            .replace(TOPIC, &topic_id)
            .replace(SCORE, &ratio(score))
            .replace(URGENCY, &ratio(urgency))
            .replace(PEOPLE, &reach.people.to_string())
            .replace(CONVERSATIONS, &reach.conversations.to_string())
            .replace(SOURCES, &reach.sources.to_string())
            .replace(STATE, states.get(&state).unwrap_or(&state))
            .replace(LATE, &delayed)
            .replace(ORIGIN, origins.get(&origin).map_or("", String::as_str))
            .replace(LABEL, &label)
            .replace(CHANGES, &changes)
            .replacen(ANALYSIS, &excerpt, 1),
    }
}

fn ratio(value: Option<f64>) -> String {
    value.map_or_default(|found| format!("{found:.2}"))
}

pub(crate) fn recap(req: RecapRequest) -> NoticeResponse {
    let RecapRequest {
        item,
        items,
        states,
        template,
    } = req;
    let lines: Vec<String> = items
        .iter()
        .map(|entry| {
            item.replace(SCORE, &ratio(entry.score))
                .replace(STATE, states.get(&entry.state).unwrap_or(&entry.state))
                .replace(LABEL, &entry.label)
        })
        .collect();
    NoticeResponse {
        text: template.replace(COUNT, &items.len().to_string()).replacen(
            ITEMS,
            &lines.join("\n"),
            1,
        ),
    }
}
