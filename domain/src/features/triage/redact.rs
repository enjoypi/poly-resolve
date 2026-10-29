use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::AppError;

pub(crate) struct Mask {
    steps: Vec<Step>,
}

impl Mask {
    pub(crate) fn apply(&self, text: &str) -> String {
        self.steps.iter().fold(text.to_owned(), |done, step| {
            step.pattern
                .replace_all(&done, step.placeholder.as_str())
                .into_owned()
        })
    }

    pub(crate) fn new(rules: &[Rule]) -> Result<Self, AppError> {
        rules
            .iter()
            .map(|rule| {
                Regex::new(&rule.pattern).map(|pattern| Step {
                    pattern,
                    placeholder: rule.placeholder.clone(),
                })
            })
            .collect::<Result<_, _>>()
            .map(|steps| Self { steps })
            .map_err(|_err| AppError::BadRequest)
    }
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageRedactRequest")]
pub(crate) struct RedactRequest {
    rules: Vec<Rule>,
    text: String,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriageRedactResponse")]
pub(crate) struct RedactResponse {
    text: String,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageRule")]
pub(crate) struct Rule {
    pattern: String,
    placeholder: String,
}

struct Step {
    pattern: Regex,
    placeholder: String,
}

pub(crate) fn redact(req: &RedactRequest) -> Result<RedactResponse, AppError> {
    Ok(RedactResponse {
        text: Mask::new(&req.rules)?.apply(&req.text),
    })
}
