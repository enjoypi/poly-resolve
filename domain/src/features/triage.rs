pub(super) mod prompt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Error, from_str, to_string};

use super::ingest::cipher::sealed;
use crate::AppError;

const KIND: &str = "predicate";
const ONE: f64 = 1.0;
const ZERO: f64 = 0.0;

#[derive(Deserialize)]
struct Answer {
    name: String,
    probability: Option<f64>,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageAskRequest")]
pub(crate) struct AskRequest {
    cipher_min_length: u32,
    input_max_chars: u32,
    message: Subject,
    model: String,
    notify_conversation_id: String,
    question: Question,
    self_sender_id: String,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriageAskResponse")]
pub(crate) struct AskResponse {
    body: Option<String>,
    skip: Option<Skip>,
}

#[derive(Serialize)]
struct Decision {
    input: String,
    model: String,
    questions: [Probe; 1],
}

#[derive(Serialize)]
struct Probe {
    instructions: String,
    #[serde(rename = "type")]
    kind: &'static str,
    name: String,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageQuestion")]
pub(crate) struct Question {
    instructions: String,
    name: String,
}

#[derive(Deserialize)]
struct Reply {
    answers: Vec<Answer>,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageScoreRequest")]
pub(crate) struct ScoreRequest {
    body: String,
    name: String,
    threshold: f64,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriageScoreResponse")]
pub(crate) struct ScoreResponse {
    probability: f64,
    verdict: Verdict,
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
    text: String,
}

#[derive(Clone, Copy, JsonSchema, Serialize)]
#[schemars(rename = "TriageVerdict")]
#[serde(rename_all = "lowercase")]
pub(crate) enum Verdict {
    Normal,
    Urgent,
}

pub(crate) fn ask(req: AskRequest) -> AskResponse {
    let skip = reason(&req);
    let body = skip.is_none().then(|| decision(req));
    AskResponse { body, skip }
}

fn bad(_err: Error) -> AppError {
    AppError::BadRequest
}

fn decision(req: AskRequest) -> String {
    let max = usize::try_from(req.input_max_chars).unwrap_or(usize::MAX);
    let decision = Decision {
        input: req.message.text.chars().take(max).collect(),
        model: req.model,
        questions: [Probe {
            instructions: req.question.instructions,
            kind: KIND,
            name: req.question.name,
        }],
    };
    to_string(&decision).unwrap_or_default()
}

fn reason(req: &AskRequest) -> Option<Skip> {
    let message = &req.message;
    let min_length = usize::try_from(req.cipher_min_length).unwrap_or(usize::MAX);
    if message.sender_id == req.self_sender_id {
        Some(Skip::Own)
    } else if message.conversation_id == req.notify_conversation_id {
        Some(Skip::Notice)
    } else if message.text.trim().is_empty() {
        Some(Skip::Empty)
    } else if sealed(&message.text, min_length) {
        Some(Skip::Sealed)
    } else {
        None
    }
}

pub(crate) fn score(req: &ScoreRequest) -> Result<ScoreResponse, AppError> {
    let reply: Reply = from_str(&req.body).map_err(bad)?;
    let probability = reply
        .answers
        .iter()
        .find(|answer| answer.name == req.name)
        .and_then(|answer| answer.probability)
        .filter(|value| (ZERO..=ONE).contains(value))
        .ok_or(AppError::BadRequest)?;
    let verdict = if probability >= req.threshold {
        Verdict::Urgent
    } else {
        Verdict::Normal
    };
    Ok(ScoreResponse {
        probability,
        verdict,
    })
}
