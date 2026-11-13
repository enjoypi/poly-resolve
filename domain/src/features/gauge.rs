use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::limit;
use crate::AppError;
use crate::features::Context;
use crate::gauge::{Day, Health};

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "UsageListRequest")]
pub(crate) struct UsageRequest {
    limit: Option<u32>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "UsageListResponse")]
pub(crate) struct UsageResponse {
    days: Vec<Day>,
}

pub(crate) fn sources(context: &Context<'_>) -> Result<Health, AppError> {
    context.gauge().sources()
}

pub(crate) fn usage(req: &UsageRequest, context: &Context<'_>) -> Result<UsageResponse, AppError> {
    let days = limit(req.limit, context)?;
    Ok(UsageResponse {
        days: context.gauge().usage(days)?,
    })
}
