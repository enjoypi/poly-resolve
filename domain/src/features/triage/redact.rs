use regex::{Captures, Regex};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::AppError;
use crate::redact::Seal;

pub(crate) const HASH: &str = "{hash}";

#[derive(Debug)]
pub(crate) struct Mask {
    seal: Option<Seal>,
    steps: Vec<Step>,
}

impl Mask {
    pub(crate) fn apply(&self, text: &str) -> String {
        self.steps.iter().fold(text.to_owned(), |done, step| {
            match (step.group, self.seal.as_ref()) {
                (Some(group), Some(seal)) => step
                    .pattern
                    .replace_all(&done, |caps: &Captures<'_>| step.swap(caps, (group, seal)))
                    .into_owned(),
                _ => step
                    .pattern
                    .replace_all(&done, step.placeholder.as_str())
                    .into_owned(),
            }
        })
    }

    pub(crate) fn new(rules: &[Rule], seal: Option<&Seal>) -> Result<Self, AppError> {
        let steps = rules
            .iter()
            .map(|rule| {
                Regex::new(&rule.pattern).map(|pattern| Step {
                    group: rule
                        .placeholder
                        .contains(HASH)
                        .then(|| rule.group.unwrap_or_default()),
                    pattern,
                    placeholder: rule.placeholder.clone(),
                })
            })
            .collect::<Result<Vec<Step>, _>>()
            .map_err(|_err| AppError::BadRequest)?;
        if seal.is_none() && steps.iter().any(|step| step.group.is_some()) {
            return Err(AppError::BadRequest);
        }
        Ok(Self {
            seal: seal.cloned(),
            steps,
        })
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
    #[serde(default)]
    group: Option<usize>,
    pattern: String,
    placeholder: String,
}

impl Rule {
    pub(crate) fn new(pattern: &str, placeholder: &str) -> Self {
        Self {
            group: None,
            pattern: pattern.to_owned(),
            placeholder: placeholder.to_owned(),
        }
    }
}

#[derive(Debug)]
struct Step {
    group: Option<usize>,
    pattern: Regex,
    placeholder: String,
}

impl Step {
    fn swap(&self, caps: &Captures<'_>, keyed: (usize, &Seal)) -> String {
        let (group, seal) = keyed;
        let value = caps.get(group).map_or("", |hit| hit.as_str());
        if self.tagged(value, seal) {
            return caps.get(0).map_or("", |hit| hit.as_str()).to_owned();
        }
        let mut shown = String::new();
        caps.expand(&self.placeholder, &mut shown);
        shown.replace(HASH, &seal.tag(value))
    }

    fn tagged(&self, value: &str, seal: &Seal) -> bool {
        let (before, tail) = self.placeholder.split_once(HASH).unwrap_or_default();
        let head = before.rsplit('}').next().unwrap_or_default();
        value
            .strip_prefix(head)
            .and_then(|rest| rest.strip_suffix(tail))
            .is_some_and(|inner| {
                inner.len() == seal.chars() && inner.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
    }
}

pub(crate) fn redact(req: &RedactRequest, seal: Option<&Seal>) -> Result<RedactResponse, AppError> {
    Ok(RedactResponse {
        text: Mask::new(&req.rules, seal)?.apply(&req.text),
    })
}
