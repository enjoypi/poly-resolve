use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::AppError;
use crate::features::Context;
use crate::inbox::{Mark, Query, Row};

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "MessagesListRequest")]
pub(crate) struct ListRequest {
    cursor: Option<String>,
    limit: Option<u32>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "MessagesListResponse")]
pub(crate) struct ListResponse {
    items: Vec<Row>,
    next_cursor: Option<String>,
}

pub(crate) fn list(req: &ListRequest, context: &Context<'_>) -> Result<ListResponse, AppError> {
    let paging = context.paging();
    let limit = req.limit.unwrap_or_else(|| paging.default_limit());
    if limit == 0 || limit > paging.max_limit() {
        return Err(AppError::BadRequest);
    }
    let cursor = req.cursor.as_deref().map(Mark::parse).transpose()?;
    let mut items = context
        .reader()
        .page(&Query::new(cursor, limit.saturating_add(1)))?;
    let size = usize::try_from(limit).unwrap_or(usize::MAX);
    let more = items.len() > size;
    items.truncate(size);
    let next_cursor = items.last().filter(|_| more).map(Row::cursor);
    Ok(ListResponse { items, next_cursor })
}
