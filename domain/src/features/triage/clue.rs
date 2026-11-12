use alloc::collections::BTreeSet;

use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::AppError;

const MAIL: &str = "mail";
const MAIL_IDS: [&str; 2] = ["in_reply_to", "message_id"];
const REFERENCES: &str = "references";
const SUBJECT: &str = "subject";

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriageClue")]
pub(crate) struct Clue {
    kind: String,
    value: String,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageCluesRequest")]
pub(crate) struct CluesRequest {
    message: Message,
    patterns: Vec<Probe>,
    subject: Subject,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriageCluesResponse")]
pub(crate) struct CluesResponse {
    clues: Vec<Clue>,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageClueMessage")]
pub(crate) struct Message {
    #[serde(default)]
    raw: Value,
    text: String,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageClueProbe")]
pub(crate) struct Probe {
    kind: String,
    pattern: String,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageClueSubject")]
pub(crate) struct Subject {
    ignore: Vec<String>,
    min_chars: u32,
    prefixes: Vec<String>,
}

impl Subject {
    fn normal(&self, subject: &str) -> Option<String> {
        let lowered = subject.trim().to_lowercase();
        let mut rest = lowered.as_str();
        while let Some(stripped) = self
            .prefixes
            .iter()
            .filter(|prefix| !prefix.is_empty())
            .find_map(|prefix| rest.strip_prefix(prefix.as_str()))
        {
            rest = stripped.trim_start();
        }
        let min = usize::try_from(self.min_chars).unwrap_or(usize::MAX);
        (rest.chars().count() >= min && !self.ignore.iter().any(|skip| skip == rest))
            .then(|| rest.to_owned())
    }
}

pub(crate) fn clues(req: &CluesRequest) -> Result<CluesResponse, AppError> {
    let mut found: BTreeSet<(String, String)> = BTreeSet::new();
    let raw = &req.message.raw;
    let ids = MAIL_IDS
        .iter()
        .filter_map(|name| raw.get(name).and_then(Value::as_str))
        .chain(
            raw.get(REFERENCES)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str),
        );
    found.extend(ids.map(|ident| (MAIL.to_owned(), ident.to_owned())));
    let subject = raw
        .get(SUBJECT)
        .and_then(Value::as_str)
        .and_then(|subject| req.subject.normal(subject));
    found.extend(subject.map(|value| (SUBJECT.to_owned(), value)));
    for rule in &req.patterns {
        let pattern = Regex::new(&rule.pattern).map_err(|_err| AppError::BadRequest)?;
        found.extend(
            pattern
                .captures_iter(&req.message.text)
                .filter_map(|caps| caps.get(1).or_else(|| caps.get(0)))
                .map(|hit| (rule.kind.clone(), hit.as_str().to_owned())),
        );
    }
    Ok(CluesResponse {
        clues: found
            .into_iter()
            .map(|(kind, value)| Clue { kind, value })
            .collect(),
    })
}
