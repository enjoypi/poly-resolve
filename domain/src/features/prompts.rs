use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::triage::pack::{LINE, REVIEW, SEGMENT};
use super::triage::prompt::MESSAGES;
use crate::AppError;
use crate::features::Context;
use crate::prompts::Prompt;

static TABLE: [Entry; 6] = [
    ("followup", &[MESSAGES]),
    ("review", &[REVIEW]),
    ("screen", &[]),
    ("session", &[MESSAGES]),
    ("topic", &[SEGMENT]),
    ("urgent", &[LINE]),
];

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "PromptsCheckRequest")]
pub(crate) struct CheckRequest {
    name: String,
    text: String,
}

type Entry = (&'static str, &'static [&'static str]);

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "PromptsKey")]
pub(crate) struct Key {
    name: String,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "PromptsListItem")]
pub(crate) struct ListItem {
    name: String,
    required: Vec<String>,
    text: String,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "PromptsListResponse")]
pub(crate) struct ListResponse {
    items: Vec<ListItem>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "PromptsNameResponse")]
pub(crate) struct NameResponse {
    name: String,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "PromptsUpdateRequest")]
pub(crate) struct UpdateRequest {
    text: String,
}

pub(crate) fn check(req: &CheckRequest) -> Result<NameResponse, AppError> {
    verify(&req.name, &req.text)?;
    Ok(NameResponse {
        name: req.name.clone(),
    })
}

pub(crate) fn list(context: &Context<'_>) -> Result<ListResponse, AppError> {
    let items = context
        .library()
        .list()?
        .iter()
        .map(|prompt| ListItem {
            name: prompt.name().to_owned(),
            required: required(prompt.name())
                .unwrap_or_default()
                .iter()
                .map(|mark| (*mark).to_owned())
                .collect(),
            text: prompt.text().to_owned(),
        })
        .collect();
    Ok(ListResponse { items })
}

fn required(name: &str) -> Option<&'static [&'static str]> {
    TABLE
        .iter()
        .find(|entry| entry.0 == name)
        .map(|entry| entry.1)
}

pub(crate) fn update(
    target: &Key,
    change: UpdateRequest,
    context: &Context<'_>,
) -> Result<NameResponse, AppError> {
    verify(&target.name, &change.text)?;
    let prompt = Prompt::new(target.name.clone(), change.text);
    context.library().save(&prompt)?.ok_or(AppError::NotFound)?;
    Ok(NameResponse {
        name: target.name.clone(),
    })
}

fn verify(name: &str, text: &str) -> Result<(), AppError> {
    let marks = required(name).ok_or(AppError::NotFound)?;
    let fits = !text.trim().is_empty() && marks.iter().all(|mark| text.matches(mark).count() == 1);
    fits.then_some(()).ok_or(AppError::Unprocessable)
}
