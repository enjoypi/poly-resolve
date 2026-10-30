use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::triage::learn;
use crate::AppError;
use crate::features::Context;
use crate::review::{Item, Listing, Reason, Rule, Ruling};

const PENDING: &str = "pending";

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "ReviewKey")]
pub(crate) struct Key {
    key: String,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "ReviewListRequest")]
pub(crate) struct ListRequest {
    limit: Option<u32>,
    state: Option<String>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "ReviewListResponse")]
pub(crate) struct ListResponse {
    items: Vec<Item>,
    reasons: Vec<Reason>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "ReviewRulingResponse")]
pub(crate) struct RulingResponse {
    state: String,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "ReviewUpdateRequest")]
#[serde(tag = "state", rename_all = "lowercase")]
pub(crate) enum Update {
    Approved,
    Rejected {
        leak: Option<String>,
        reason: String,
    },
}

pub(crate) fn list(req: &ListRequest, context: &Context<'_>) -> Result<ListResponse, AppError> {
    let paging = context.paging();
    let limit = req.limit.unwrap_or_else(|| paging.default_limit());
    if limit == 0 || limit > paging.max_limit() {
        return Err(AppError::BadRequest);
    }
    let state = req.state.as_deref().unwrap_or(PENDING);
    Ok(ListResponse {
        items: context.docket().list(&Listing::new(state, limit))?,
        reasons: context.reasons().to_vec(),
    })
}

fn reject(
    context: &Context<'_>,
    ruling: Ruling<'_>,
    leak: Option<&str>,
) -> Result<RulingResponse, AppError> {
    let reason = context
        .reasons()
        .iter()
        .find(|known| known.code() == ruling.reason())
        .ok_or(AppError::Unprocessable)?;
    let pasted = leak.map(str::trim).filter(|text| !text.is_empty());
    match (reason.placeholder(), pasted) {
        (Some(placeholder), Some(raw)) => {
            let text = context
                .docket()
                .source(ruling.key())?
                .ok_or(AppError::NotFound)?;
            let pattern = learn::rule(raw, &text, context.learn_min_chars())?;
            let rule = Rule::new(pattern, placeholder.to_owned());
            settle(context, &ruling.learned(&rule))
        }
        (None, None) => settle(context, &ruling),
        _ => Err(AppError::Unprocessable),
    }
}

fn settle(context: &Context<'_>, ruling: &Ruling<'_>) -> Result<RulingResponse, AppError> {
    let state = context.docket().settle(ruling)?.ok_or(AppError::NotFound)?;
    if state == ruling.state() {
        Ok(RulingResponse { state })
    } else {
        Err(AppError::Conflict)
    }
}

pub(crate) fn update(
    target: &Key,
    change: Update,
    context: &Context<'_>,
) -> Result<RulingResponse, AppError> {
    match change {
        Update::Approved => settle(context, &Ruling::approve(&target.key)),
        Update::Rejected { leak, reason } => reject(
            context,
            Ruling::reject(&target.key, &reason),
            leak.as_deref(),
        ),
    }
}
