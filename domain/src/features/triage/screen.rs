use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json, to_string};

use super::pack::{Open, alias};

const SYSTEM: &str = "system";
const TEMPERATURE: f64 = 0.0;
const USER: &str = "user";

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageCandidate")]
pub(crate) struct Candidate {
    key: String,
    text: String,
}

impl Candidate {
    pub(super) fn key(&self) -> &str {
        &self.key
    }

    pub(super) fn text(&self) -> &str {
        &self.text
    }
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageScreenRequest")]
pub(crate) struct ScreenRequest {
    instructions: String,
    lines: Vec<Candidate>,
    model: String,
    topics: Vec<Open>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriageScreenResponse")]
pub(crate) struct ScreenResponse {
    body: String,
}

pub(crate) fn screen(req: ScreenRequest) -> ScreenResponse {
    let ScreenRequest {
        instructions,
        lines,
        model,
        topics,
    } = req;
    let shown: Vec<Value> = lines
        .iter()
        .enumerate()
        .map(|(index, line)| json!({"line": alias("L", index), "text": line.text}))
        .collect();
    let known: Vec<Value> = topics
        .iter()
        .map(|open| json!({"label": open.label(), "topic": open.topic()}))
        .collect();
    let body = json!({
        "messages": [
            {"content": instructions, "role": SYSTEM},
            {"content": json!({"lines": shown, "topics": known}).to_string(), "role": USER},
        ],
        "model": model,
        "response_format": {"type": "json_object"},
        "temperature": TEMPERATURE,
    });
    ScreenResponse {
        body: to_string(&body).unwrap_or_default(),
    }
}
