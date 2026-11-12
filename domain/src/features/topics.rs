use schemars::JsonSchema;
use serde::Deserialize;

use super::limit;
use crate::AppError;
use crate::features::Context;
use crate::outbound::Listing;
use crate::topics::{Detail, Overview};

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TopicKey")]
pub(crate) struct Key {
    topic_id: String,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TopicListRequest")]
pub(crate) struct ListRequest {
    limit: Option<u32>,
    state: Option<String>,
}

pub(crate) fn get(target: &Key, context: &Context<'_>) -> Result<Detail, AppError> {
    context
        .board()
        .detail(&target.topic_id)?
        .ok_or(AppError::NotFound)
}

pub(crate) fn list(req: &ListRequest, context: &Context<'_>) -> Result<Overview, AppError> {
    let count = limit(req.limit, context)?;
    let state = req.state.as_deref().unwrap_or_default();
    context.board().overview(&Listing::new(state, count))
}
