use std::fs;
use std::io;

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
fn spec_declares_storage_failure_for_messages() {
    let document = document().unwrap();
    let statuses = |path: &str| -> Vec<String> {
        document
            .pointer(path)
            .and_then(Value::as_object)
            .map_or_default(|responses| responses.keys().cloned().collect())
    };
    assert_eq!(
        statuses("/paths/~1messages~1list/post/responses"),
        ["200", "400", "503"]
    );
    assert_eq!(
        document.pointer("/paths/~1messages~1list/post/responses/503/description"),
        Some(&json!("storage unavailable"))
    );
    assert_eq!(
        statuses("/paths/~1health~1check/post/responses"),
        ["200", "400"]
    );
}

#[test]
fn spec_describes_health_check_operation() {
    let header = |name: &str| {
        json!({
            "description": "set by the gateway",
            "in": "header",
            "name": name,
            "required": false,
            "schema": {"type": "string"}
        })
    };
    let body = |schema: Value, description: &str| json!({"content": {"application/json": {"schema": schema}}, "description": description});
    assert_eq!(
        document().unwrap().pointer("/paths/~1health~1check/post"),
        Some(&json!({
            "parameters": [header("x-session-id"), header("x-user")],
            "requestBody": {
                "content": {
                    "application/json": {
                        "schema": {"additionalProperties": true, "type": "object"}
                    }
                },
                "required": true
            },
            "responses": {
                "200": body(
                    json!({"$ref": "#/components/schemas/HealthCheckResponse"}),
                    "success"
                ),
                "400": body(json!({"$ref": "#/components/schemas/ErrorBody"}), "bad request")
            }
        }))
    );
}

#[test]
fn spec_lists_schemas() {
    let document = document().unwrap();
    let schemas = document
        .pointer("/components/schemas")
        .and_then(Value::as_object)
        .unwrap();
    assert_eq!(
        schemas.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "ErrorBody",
            "HealthCheckResponse",
            "IngestAssembleRequest",
            "IngestChild",
            "IngestCipherRequest",
            "IngestCipherResponse",
            "IngestEnvelope",
            "IngestParsePageResponse",
            "IngestParseRequest",
            "IngestRevealRequest",
            "IngestWindowRequest",
            "IngestWindowResponse",
            "MessagesListItem",
            "MessagesListRequest",
            "MessagesListResponse"
        ]
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
