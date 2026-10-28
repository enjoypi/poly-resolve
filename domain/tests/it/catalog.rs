use domain::features::catalog::build;
use serde_json::json;

use crate::support::{Fixture, cli};

static ROUTES: [(&str, &str); 12] = [
    ("health", "check"),
    ("ingest", "assemble"),
    ("ingest", "cipher"),
    ("ingest", "parse_event"),
    ("ingest", "parse_page"),
    ("ingest", "reveal"),
    ("ingest", "window"),
    ("messages", "list"),
    ("triage", "ask"),
    ("triage", "brief"),
    ("triage", "notice"),
    ("triage", "score"),
];
static SCHEMAS: [&str; 28] = [
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
    "MessagesListResponse",
    "TriageAskRequest",
    "TriageAskResponse",
    "TriageBriefRequest",
    "TriageBriefResponse",
    "TriageLine",
    "TriageNoticeRequest",
    "TriageNoticeResponse",
    "TriageQuestion",
    "TriageScoreRequest",
    "TriageScoreResponse",
    "TriageSkip",
    "TriageSubject",
    "TriageVerdict",
];

type Failure<'name> = (&'name str, Vec<u16>);

#[test]
fn describes_health_route() {
    let listing = build();
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
        listing.schemas().get("HealthCheckResponse"),
        Some(&json!({
            "properties": {"status": {"type": "string"}},
            "required": ["status"],
            "type": "object"
        }))
    );
}

#[test]
fn lists_failures_per_route() {
    let failing: Vec<Failure<'_>> = build()
        .entries()
        .iter()
        .map(|entry| {
            (
                entry.feature(),
                entry
                    .failures()
                    .iter()
                    .map(|err| err.status())
                    .collect::<Vec<u16>>(),
            )
        })
        .filter(|entry| !entry.1.is_empty())
        .collect();
    assert_eq!(failing, [("messages", vec![503])]);
}

#[test]
fn lists_routes() {
    let listing = build();
    let routes: Vec<(&str, &str)> = listing
        .entries()
        .iter()
        .map(|entry| (entry.feature(), entry.action()))
        .collect();
    assert_eq!(routes, ROUTES);
}

#[test]
fn lists_schemas() {
    let listing = build();
    let names: Vec<&str> = listing.schemas().keys().map(String::as_str).collect();
    assert_eq!(names, SCHEMAS);
}

#[test]
fn unknown_action_of_known_feature_is_rejected() {
    let err = Fixture::new()
        .unwrap()
        .dispatch(&cli("health", "nope", "{}"))
        .unwrap_err();
    assert_eq!(err.status(), 404);
}
