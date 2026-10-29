use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::redact::{Mask, Rule};
use super::transcript::{Row, Style, fit};
use crate::AppError;

const ANALYSIS: &str = "{analysis}";
const MESSAGES: &str = "{messages}";
const PROBABILITY: &str = "{probability}";
const TOPIC: &str = "{topic}";

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageBriefRequest")]
pub(crate) struct BriefRequest {
    followup: String,
    max_chars: u32,
    resume: Option<Resume>,
    rows: Vec<Row>,
    rules: Vec<Rule>,
    template: String,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriageBriefResponse")]
pub(crate) struct BriefResponse {
    prompt: String,
    start: Start,
    until_ms: u64,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageNoticeRequest")]
pub(crate) struct NoticeRequest {
    analysis: String,
    max_chars: u32,
    probability: f64,
    template: String,
    topic_id: String,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriageNoticeResponse")]
pub(crate) struct NoticeResponse {
    text: String,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageResume")]
pub(crate) struct Resume {
    until: u64,
}

#[derive(Clone, Copy, JsonSchema, Serialize)]
#[schemars(rename = "TriageStart")]
#[serde(rename_all = "lowercase")]
pub(crate) enum Start {
    Fresh,
    Resume,
}

pub(crate) fn brief(req: &BriefRequest) -> Result<BriefResponse, AppError> {
    let mask = Mask::new(&req.rules)?;
    let max = usize::try_from(req.max_chars).unwrap_or(usize::MAX);
    let style = Style::new(&mask, max);
    let after = req.resume.as_ref().map(|point| point.until);
    let (template, start) = after.map_or((&req.template, Start::Fresh), |_| {
        (&req.followup, Start::Resume)
    });
    let mut sorted: Vec<&Row> = req.rows.iter().collect();
    sorted.sort_by_key(|row| row.occurred_at_ms());
    let lines = sorted
        .into_iter()
        .filter(|row| after.is_none_or(|until| row.occurred_at_ms() > until))
        .map(|row| row.render(&style))
        .collect();
    Ok(BriefResponse {
        prompt: template.replacen(MESSAGES, &fit(lines, max), 1),
        start,
        until_ms: req
            .rows
            .iter()
            .map(Row::occurred_at_ms)
            .max()
            .unwrap_or_default(),
    })
}

pub(crate) fn notice(req: NoticeRequest) -> NoticeResponse {
    let NoticeRequest {
        analysis,
        max_chars,
        probability,
        template,
        topic_id,
    } = req;
    let max = usize::try_from(max_chars).unwrap_or(usize::MAX);
    let excerpt: String = analysis.chars().take(max).collect();
    NoticeResponse {
        text: template
            .replace(TOPIC, &topic_id)
            .replace(PROBABILITY, &format!("{probability:.2}"))
            .replacen(ANALYSIS, &excerpt, 1),
    }
}
