use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::triage::learn;
use crate::AppError;
use crate::features::Context;
use crate::review::{Item, Listing, Reason, Rule, Ruling};

const PENDING: &str = "pending";

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "ReviewApproveRequest")]
pub(crate) struct ApproveRequest {
    keys: Vec<String>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "ReviewApproveResponse")]
pub(crate) struct ApproveResponse {
    states: Vec<String>,
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

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "ReviewRejectRequest")]
pub(crate) struct RejectRequest {
    key: String,
    leak: Option<String>,
    reason: String,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "ReviewRulingResponse")]
pub(crate) struct RulingResponse {
    state: String,
}

pub(crate) fn approve(
    req: &ApproveRequest,
    context: &Context<'_>,
) -> Result<ApproveResponse, AppError> {
    let states = req
        .keys
        .iter()
        .map(|key| settle(context, &Ruling::approve(key)).map(|done| done.state))
        .collect::<Result<Vec<String>, AppError>>()?;
    Ok(ApproveResponse { states })
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

pub(crate) fn reject(
    req: &RejectRequest,
    context: &Context<'_>,
) -> Result<RulingResponse, AppError> {
    let reason = context
        .reasons()
        .iter()
        .find(|known| known.code() == req.reason)
        .ok_or(AppError::BadRequest)?;
    let leak = req
        .leak
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty());
    let ruling = Ruling::reject(&req.key, &req.reason);
    match (reason.placeholder(), leak) {
        (Some(placeholder), Some(raw)) => {
            let text = context
                .docket()
                .source(&req.key)?
                .ok_or(AppError::BadRequest)?;
            let pattern = learn::rule(raw, &text, context.learn_min_chars())?;
            let rule = Rule::new(pattern, placeholder.to_owned());
            settle(context, &ruling.learned(&rule))
        }
        (None, None) => settle(context, &ruling),
        _ => Err(AppError::BadRequest),
    }
}

fn settle(context: &Context<'_>, ruling: &Ruling<'_>) -> Result<RulingResponse, AppError> {
    context
        .docket()
        .settle(ruling)?
        .map(|state| RulingResponse { state })
        .ok_or(AppError::BadRequest)
}
