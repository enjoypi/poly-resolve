use domain::features::catalog::build;
use serde_json::json;

use crate::support::{Fixture, local};

static ROUTES: [Bound; 4] = [
    ("health", "check", "get", "/api/health"),
    ("messages", "list", "get", "/api/messages"),
    ("review", "list", "get", "/api/reviews"),
    ("review", "update", "patch", "/api/reviews/{key}"),
];
static SCHEMAS: [&str; 12] = [
    "ErrorBody",
    "HealthCheckResponse",
    "MessagesListItem",
    "MessagesListRequest",
    "MessagesListResponse",
    "ReviewItem",
    "ReviewKey",
    "ReviewListRequest",
    "ReviewListResponse",
    "ReviewReason",
    "ReviewRulingResponse",
    "ReviewUpdateRequest",
];

type Bound = (&'static str, &'static str, &'static str, &'static str);

type Failure<'name> = (&'name str, Vec<u16>);

#[test]
fn describes_health_route() {
    let listing = build();
    let entry = listing.entries().first().unwrap();
    assert_eq!(entry.params(), None);
    assert_eq!(entry.body(), None);
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
fn describes_review_update_route() {
    let listing = build();
    let entry = listing.entries().get(3).unwrap();
    assert_eq!(
        entry.params(),
        Some(&json!({"$ref": "#/components/schemas/ReviewKey"}))
    );
    assert_eq!(
        entry.body(),
        Some(&json!({"$ref": "#/components/schemas/ReviewUpdateRequest"}))
    );
    let update = listing.schemas().get("ReviewUpdateRequest").unwrap();
    assert_eq!(
        update
            .get("oneOf")
            .and_then(|all| all.as_array())
            .map(Vec::len),
        Some(2)
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
        .collect();
    assert_eq!(
        failing,
        [
            ("health", vec![]),
            ("messages", vec![400, 503]),
            ("review", vec![400, 503]),
            ("review", vec![400, 404, 409, 422, 503]),
        ]
    );
}

#[test]
fn lists_routes() {
    let listing = build();
    let routes: Vec<Bound> = listing
        .entries()
        .iter()
        .map(|entry| {
            (
                entry.feature(),
                entry.action(),
                entry.method(),
                entry.path(),
            )
        })
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
        .dispatch(&local("health", "nope", "{}"))
        .unwrap_err();
    assert_eq!(err.status(), 404);
}
