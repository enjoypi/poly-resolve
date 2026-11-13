use regex::{Regex, escape};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Error, from_str};

use super::pack::{Open, alias};
use super::redact::HASH;
use super::screen::Candidate;
use crate::AppError;

const HIGH: &str = "high";
const MISSING: &str = "missing";
const UNMASKED: &str = "unmasked";

#[derive(Deserialize)]
struct Completion {
    choices: Vec<Reply>,
}

struct Judge<'judge> {
    guard: Option<Regex>,
    req: &'judge ScreenedRequest,
}

impl Judge<'_> {
    fn hints(&self, found: &Verdict) -> Vec<String> {
        let mut hints: Vec<String> = Vec::new();
        for name in &found.topics {
            let known = (0..self.req.topics.len())
                .find(|index| alias("T", *index) == *name)
                .and_then(|index| self.req.topics.get(index))
                .map(|open| open.topic().to_owned());
            if let Some(topic) = known
                && !hints.contains(&topic)
            {
                hints.push(topic);
            }
        }
        hints
    }

    fn rule(&self, line: &Candidate, verdict: Option<&Verdict>) -> Screened {
        let Some(found) = verdict else {
            return Screened {
                flag: MISSING.to_owned(),
                hints: Vec::new(),
                key: line.key().to_owned(),
                spans: Vec::new(),
                text: line.text().to_owned(),
            };
        };
        let (spans, stale) = self.spans(line, found);
        let text = spans.iter().fold(line.text().to_owned(), |done, span| {
            done.replace(span, &self.req.placeholder)
        });
        Screened {
            flag: if found.risk == HIGH && spans.is_empty() && !stale {
                UNMASKED.to_owned()
            } else {
                String::new()
            },
            hints: self.hints(found),
            key: line.key().to_owned(),
            spans: spans.iter().map(|span| (*span).to_owned()).collect(),
            text,
        }
    }

    fn spans<'found>(&self, line: &Candidate, found: &'found Verdict) -> Picked<'found> {
        let min = usize::try_from(self.req.min_chars)
            .unwrap_or(usize::MAX)
            .max(1);
        let valid: Vec<&str> = found
            .spans
            .iter()
            .map(|span| span.as_str().trim())
            .filter(|span| span.chars().count() >= min && line.text().contains(span))
            .collect();
        let spans: Vec<&str> = valid
            .iter()
            .copied()
            .filter(|span| {
                !self
                    .guard
                    .as_ref()
                    .is_some_and(|guard| guard.is_match(span))
            })
            .collect();
        let stale = spans.len() < valid.len();
        (spans, stale)
    }
}

#[derive(Deserialize)]
struct Message {
    content: String,
}

type Picked<'found> = (Vec<&'found str>, bool);

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
    spans: Vec<String>,
    text: String,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageScreenedRequest")]
pub(crate) struct ScreenedRequest {
    lines: Vec<Candidate>,
    #[serde(default)]
    marks: Vec<String>,
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

fn guard(marks: &[String]) -> Option<Regex> {
    let hash = escape(HASH);
    let joined = (!marks.is_empty()).then(|| {
        marks
            .iter()
            .map(|mark| escape(mark).replace(&hash, "[0-9a-f]+"))
            .collect::<Vec<_>>()
            .join("|")
    })?;
    Regex::new(&joined).ok()
}

pub(crate) fn screened(req: &ScreenedRequest) -> Result<ScreenedResponse, AppError> {
    let completion: Completion = from_str(&req.response).map_err(bad)?;
    let reply = completion.choices.first().ok_or(AppError::BadRequest)?;
    let verdicts: Verdicts = from_str(&reply.message.content).map_err(bad)?;
    let judge = Judge {
        guard: guard(&req.marks),
        req,
    };
    let lines = req
        .lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let name = alias("L", index);
            let verdict = verdicts.lines.iter().find(|found| found.line == name);
            judge.rule(line, verdict)
        })
        .collect();
    Ok(ScreenedResponse { lines })
}
