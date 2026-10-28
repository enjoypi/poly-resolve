use domain::AppError;
use domain::features::catalog::{Entry, Listing, build};
use serde_json::{Map, Value, json, to_string_pretty};

use crate::http::{SESSION_HEADER, USER_HEADER};

fn header(name: &str) -> Value {
    json!({
        "description": "set by the gateway",
        "in": "header",
        "name": name,
        "required": false,
        "schema": {"type": "string"}
    })
}

fn operation(entry: &Entry, listing: &Listing) -> Value {
    json!({
        "parameters": [header(SESSION_HEADER), header(USER_HEADER)],
        "requestBody": {
            "content": {"application/json": {"schema": entry.request()}},
            "required": true
        },
        "responses": {
            "200": {
                "content": {"application/json": {"schema": entry.response()}},
                "description": "success"
            },
            AppError::BadRequest.status().to_string(): {
                "content": {"application/json": {"schema": listing.error()}},
                "description": "bad request"
            }
        }
    })
}

pub(crate) fn render() -> String {
    let listing = build();
    let paths: Map<String, Value> = listing
        .entries()
        .iter()
        .map(|entry| {
            (
                format!("/{}/{}", entry.feature(), entry.action()),
                json!({"post": operation(entry, &listing)}),
            )
        })
        .collect();
    let document = json!({
        "components": {"schemas": listing.schemas()},
        "info": {"title": env!("CARGO_PKG_NAME"), "version": env!("CARGO_PKG_VERSION")},
        "openapi": "3.1.0",
        "paths": paths
    });
    to_string_pretty(&document).unwrap_or_default()
}
