use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Error, from_str};

use super::pack::{Open, alias};
use super::screen::Candidate;
use crate::AppError;

const HIGH: &str = "high";
const MISSING: &str = "missing";
const UNMASKED: &str = "unmasked";

#[derive(Deserialize)]
struct Completion {
    choices: Vec<Reply>,
}

#[derive(Deserialize)]
struct Message {
    content: String,
}

#[derive(Deserialize)]
struct Reply {
    message: Message,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriageScreened")]
pub(crate) struct Screened {
    flag: String,
    hints: Vec<String>,
    key: String,
    text: String,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageScreenedRequest")]
pub(crate) struct ScreenedRequest {
    lines: Vec<Candidate>,
    min_chars: u32,
    placeholder: String,
    response: String,
    topics: Vec<Open>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriageScreenedResponse")]
pub(crate) struct ScreenedResponse {
    lines: Vec<Screened>,
}

#[derive(Deserialize)]
struct Verdict {
    line: String,
    risk: String,
    spans: Vec<String>,
    topics: Vec<String>,
}

#[derive(Deserialize)]
struct Verdicts {
    lines: Vec<Verdict>,
}

fn bad(_err: Error) -> AppError {
    AppError::BadRequest
}

fn judge(req: &ScreenedRequest, line: &Candidate, verdict: Option<&Verdict>) -> Screened {
    let Some(found) = verdict else {
        return Screened {
            flag: MISSING.to_owned(),
            hints: Vec::new(),
            key: line.key().to_owned(),
            text: line.text().to_owned(),
        };
    };
    let min = usize::try_from(req.min_chars).unwrap_or(usize::MAX).max(1);
    let spans: Vec<&str> = found
        .spans
        .iter()
        .map(|span| span.as_str().trim())
        .filter(|span| span.chars().count() >= min && line.text().contains(span))
        .collect();
    let text = spans.iter().fold(line.text().to_owned(), |done, span| {
        done.replace(span, &req.placeholder)
    });
    let mut hints: Vec<String> = Vec::new();
    for topic in &found.topics {
        let known = req.topics.iter().any(|open| open.topic() == topic);
        if known && !hints.contains(topic) {
            hints.push(topic.clone());
        }
    }
    Screened {
        flag: if found.risk == HIGH && spans.is_empty() {
            UNMASKED.to_owned()
        } else {
            String::new()
        },
        hints,
        key: line.key().to_owned(),
        text,
    }
}

pub(crate) fn screened(req: &ScreenedRequest) -> Result<ScreenedResponse, AppError> {
    let completion: Completion = from_str(&req.response).map_err(bad)?;
    let reply = completion.choices.first().ok_or(AppError::BadRequest)?;
    let verdicts: Verdicts = from_str(&reply.message.content).map_err(bad)?;
    let lines = req
        .lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let name = alias("L", index);
            let verdict = verdicts.lines.iter().find(|found| found.line == name);
            judge(req, line, verdict)
        })
        .collect();
    Ok(ScreenedResponse { lines })
}
