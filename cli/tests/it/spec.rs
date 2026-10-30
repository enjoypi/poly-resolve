use std::fs;
use std::io;

use domain::features::catalog::build;
use serde_json::{Value, from_slice, json};

use crate::support::spec;

fn document() -> io::Result<Value> {
    let out = spec()?;
    from_slice(&out.stdout).map_err(io::Error::from)
}

#[test]
fn spec_declares_openapi_and_info() {
    let document = document().unwrap();
    assert_eq!(document.pointer("/openapi"), Some(&json!("3.1.0")));
    assert_eq!(
        document.pointer("/info"),
        Some(&json!({"title": "cli", "version": env!("CARGO_PKG_VERSION")}))
    );
}

#[test]
fn spec_declares_statuses_per_operation() {
    let document = document().unwrap();
    let statuses = |path: &str| -> Vec<String> {
        document
            .pointer(path)
            .and_then(Value::as_object)
            .map_or_default(|responses| responses.keys().cloned().collect())
    };
    assert_eq!(
        statuses("/paths/~1api~1messages/get/responses"),
        ["200", "400", "403", "503"]
    );
    assert_eq!(
        statuses("/paths/~1api~1reviews~1{key}/patch/responses"),
        ["200", "400", "403", "404", "409", "415", "422", "503"]
    );
    assert_eq!(
        document.pointer("/paths/~1api~1messages/get/responses/503/description"),
        Some(&json!("storage unavailable"))
    );
    let paths: Vec<&String> = document
        .pointer("/paths")
        .and_then(Value::as_object)
        .map_or_default(|paths| paths.keys().collect());
    assert_eq!(
        paths,
        [
            "/api/health",
            "/api/messages",
            "/api/reviews",
            "/api/reviews/{key}"
        ]
    );
}

#[test]
fn spec_describes_health_and_parameters() {
    let header = |name: &str| {
        json!({
            "description": "set by the gateway",
            "in": "header",
            "name": name,
            "required": false,
            "schema": {"type": "string"}
        })
    };
    let document = document().unwrap();
    assert_eq!(
        document.pointer("/paths/~1api~1health/get"),
        Some(&json!({
            "parameters": [header("x-session-id"), header("x-user")],
            "responses": {
                "200": {
                    "content": {"application/json": {"schema": {"$ref": "#/components/schemas/HealthCheckResponse"}}},
                    "description": "success"
                },
                "403": {"description": "host not allowed"}
            }
        }))
    );
    assert_eq!(
        document.pointer("/paths/~1api~1reviews~1{key}/patch/parameters/2"),
        Some(&json!({"in": "path", "name": "key", "required": true, "schema": {"type": "string"}}))
    );
    assert_eq!(
        document.pointer("/paths/~1api~1reviews/get/parameters/3/in"),
        Some(&json!("query"))
    );
}

#[test]
fn spec_lists_schemas() {
    let document = document().unwrap();
    let schemas = document
        .pointer("/components/schemas")
        .and_then(Value::as_object)
        .unwrap();
    let listing = build();
    let expected: Vec<&str> = listing.schemas().keys().map(String::as_str).collect();
    assert_eq!(
        schemas.keys().map(String::as_str).collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        schemas.get("ErrorBody"),
        Some(&json!({
            "properties": {"code": {"type": "string"}},
            "required": ["code"],
            "type": "object"
        }))
    );
    assert_eq!(
        schemas.get("HealthCheckResponse"),
        Some(&json!({
            "properties": {"status": {"type": "string"}},
            "required": ["status"],
            "type": "object"
        }))
    );
}

#[test]
fn spec_matches_committed_file() {
    let committed =
        fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../api/openapi.json")).unwrap();
    assert_eq!(
        String::from_utf8(spec().unwrap().stdout).unwrap(),
        committed,
        "openapi.json is stale, run bun scripts/gen.ts"
    );
}
