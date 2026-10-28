use domain::features::catalog::build;
use domain::features::dispatch;
use serde_json::json;

use crate::support::{cli, errors};

#[test]
fn lists_routes_with_schemas() {
    let listing = build();
    let routes: Vec<(&str, &str)> = listing
        .entries()
        .iter()
        .map(|entry| (entry.feature(), entry.action()))
        .collect();
    assert_eq!(
        routes,
        [
            ("health", "check"),
            ("ingest", "parse_event"),
            ("ingest", "parse_page"),
            ("ingest", "window")
        ]
    );
    let entry = listing.entries().first().unwrap();
    assert_eq!(
        entry.request(),
        &json!({"additionalProperties": true, "type": "object"})
    );
    assert_eq!(
        entry.response(),
        &json!({"$ref": "#/components/schemas/HealthCheckResponse"})
    );
    assert_eq!(
        listing.error(),
        &json!({"$ref": "#/components/schemas/ErrorBody"})
    );
    assert_eq!(
        listing
            .schemas()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [
            "ErrorBody",
            "HealthCheckResponse",
            "IngestEnvelope",
            "IngestParsePageResponse",
            "IngestParseRequest",
            "IngestWindowRequest",
            "IngestWindowResponse"
        ]
    );
    assert_eq!(
        listing.schemas().get("HealthCheckResponse"),
        Some(&json!({
            "properties": {"status": {"type": "string"}},
            "required": ["status"],
            "type": "object"
        }))
    );
}

#[test]
fn unknown_action_of_known_feature_is_rejected() {
    let errors = errors().unwrap();
    let err = dispatch(&errors, &cli("health", "nope", "{}")).unwrap_err();
    assert_eq!(err.status(), 404);
}
