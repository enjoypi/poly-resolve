use domain::features::catalog::build;
use serde_json::json;

use crate::support::{Fixture, cli};

static ROUTES: [(&str, &str); 20] = [
    ("health", "check"),
    ("ingest", "assemble"),
    ("ingest", "cipher"),
    ("ingest", "parse_event"),
    ("ingest", "parse_page"),
    ("ingest", "reveal"),
    ("ingest", "window"),
    ("messages", "list"),
    ("review", "approve"),
    ("review", "list"),
    ("review", "reject"),
    ("triage", "brief"),
    ("triage", "line"),
    ("triage", "notice"),
    ("triage", "pack"),
    ("triage", "redact"),
    ("triage", "reply"),
    ("triage", "screen"),
    ("triage", "screened"),
    ("triage", "sort"),
];
static SCHEMAS: [&str; 61] = [
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
    "ReviewApproveRequest",
    "ReviewApproveResponse",
    "ReviewItem",
    "ReviewListRequest",
    "ReviewListResponse",
    "ReviewReason",
    "ReviewRejectRequest",
    "ReviewRulingResponse",
    "TriageAttach",
    "TriageBriefRequest",
    "TriageBriefResponse",
    "TriageCandidate",
    "TriageChoice",
    "TriageEntry",
    "TriageLayout",
    "TriageLineRequest",
    "TriageLineResponse",
    "TriageNoticeRequest",
    "TriageNoticeResponse",
    "TriageOpen",
    "TriagePackRequest",
    "TriagePackResponse",
    "TriagePlaced",
    "TriagePlacement",
    "TriageQuestion",
    "TriageQuestions",
    "TriageRedactRequest",
    "TriageRedactResponse",
    "TriageReplyRequest",
    "TriageReplyResponse",
    "TriageResume",
    "TriageRow",
    "TriageRule",
    "TriageScreenRequest",
    "TriageScreenResponse",
    "TriageScreened",
    "TriageScreenedRequest",
    "TriageScreenedResponse",
    "TriageSegment",
    "TriageSkip",
    "TriageSortRequest",
    "TriageSortResponse",
    "TriageStart",
    "TriageSubject",
    "TriageTouched",
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
    assert_eq!(
        failing,
        [
            ("messages", vec![503]),
            ("review", vec![503]),
            ("review", vec![503]),
            ("review", vec![503]),
        ]
    );
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
