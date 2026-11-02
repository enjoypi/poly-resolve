use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::review::limit;
use crate::AppError;
use crate::batches::Item;
use crate::features::Context;
use crate::review::Listing;

const FAILED: &str = "failed";
const READY: &str = "ready";

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "BatchKey")]
pub(crate) struct Key {
    batch_id: u64,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "BatchListRequest")]
pub(crate) struct ListRequest {
    limit: Option<u32>,
    state: Option<String>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "BatchListResponse")]
pub(crate) struct ListResponse {
    items: Vec<Item>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "BatchStateResponse")]
pub(crate) struct StateResponse {
    state: String,
}

#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[schemars(rename = "BatchUpdateRequest")]
#[serde(tag = "state", rename_all = "lowercase")]
pub(crate) enum Update {
    Ready,
}

pub(crate) fn list(req: &ListRequest, context: &Context<'_>) -> Result<ListResponse, AppError> {
    let count = limit(req.limit, context)?;
    let state = req.state.as_deref().unwrap_or(FAILED);
    Ok(ListResponse {
        items: context.roster().list(&Listing::new(state, count))?,
    })
}

pub(crate) fn update(
    target: &Key,
    change: Update,
    context: &Context<'_>,
) -> Result<StateResponse, AppError> {
    let Update::Ready = change;
    let state = context
        .roster()
        .revive(target.batch_id)?
        .ok_or(AppError::NotFound)?;
    if state == READY {
        Ok(StateResponse { state })
    } else {
        Err(AppError::Conflict)
    }
}
