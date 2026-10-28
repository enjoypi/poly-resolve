use domain::features::catalog::build;
use serde_json::json;

use crate::support::{Fixture, cli};

type Failure<'name> = (&'name str, Vec<u16>);

#[test]
fn lists_failures_per_route() {
    let failures: Vec<Failure<'_>> = build()
        .entries()
        .iter()
        .map(|entry| {
            (
                entry.feature(),
                entry.failures().iter().map(|err| err.status()).collect(),
            )
        })
        .collect();
    assert_eq!(
        failures,
        [
            ("health", vec![]),
            ("ingest", vec![]),
            ("ingest", vec![]),
            ("ingest", vec![]),
            ("messages", vec![503])
        ]
    );
}

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
            ("ingest", "window"),
            ("messages", "list")
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
            "IngestWindowResponse",
            "MessagesListItem",
            "MessagesListRequest",
            "MessagesListResponse"
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
    let err = Fixture::new()
        .unwrap()
        .dispatch(&cli("health", "nope", "{}"))
        .unwrap_err();
    assert_eq!(err.status(), 404);
}
