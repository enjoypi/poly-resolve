use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::limit;
use crate::AppError;
use crate::features::Context;
use crate::outbound::{Item, Listing};

const APPROVED: &str = "approved";

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "OutboundListRequest")]
pub(crate) struct ListRequest {
    limit: Option<u32>,
    state: Option<String>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "OutboundListResponse")]
pub(crate) struct ListResponse {
    items: Vec<Item>,
    placeholders: Vec<String>,
}

pub(crate) fn list(req: &ListRequest, context: &Context<'_>) -> Result<ListResponse, AppError> {
    let count = limit(req.limit, context)?;
    let state = req.state.as_deref().unwrap_or(APPROVED);
    Ok(ListResponse {
        items: context.outbox().list(&Listing::new(state, count))?,
        placeholders: context.placeholders().to_vec(),
    })
}
