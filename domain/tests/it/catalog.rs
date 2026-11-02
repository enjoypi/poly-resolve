use domain::features::catalog::build;
use serde_json::json;

use crate::support::{Fixture, local};

static ROUTES: [Bound; 9] = [
    ("batches", "list", "get", "/api/batches"),
    ("batches", "update", "patch", "/api/batches/{batch_id}"),
    ("health", "check", "get", "/api/health"),
    ("messages", "list", "get", "/api/messages"),
    ("prompts", "list", "get", "/api/prompts"),
    ("prompts", "update", "patch", "/api/prompts/{name}"),
    ("review", "draw", "patch", "/api/reviews"),
    ("review", "list", "get", "/api/reviews"),
    ("review", "update", "patch", "/api/reviews/{key}"),
];
static SCHEMAS: [&str; 25] = [
    "BatchItem",
    "BatchKey",
    "BatchListRequest",
    "BatchListResponse",
    "BatchStateResponse",
    "BatchUpdateRequest",
    "ErrorBody",
    "HealthCheckResponse",
    "MessagesListItem",
    "MessagesListRequest",
    "MessagesListResponse",
    "PromptsKey",
    "PromptsListItem",
    "PromptsListResponse",
    "PromptsNameResponse",
    "PromptsUpdateRequest",
    "ReviewDrawRequest",
    "ReviewDrawResponse",
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
    let entry = listing.entries().get(2).unwrap();
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
fn describes_review_draw_route() {
    let listing = build();
    let entry = listing.entries().get(6).unwrap();
    assert_eq!(entry.params(), None);
    assert_eq!(
        entry.body(),
        Some(&json!({"$ref": "#/components/schemas/ReviewDrawRequest"}))
    );
    assert_eq!(
        entry.response(),
        &json!({"$ref": "#/components/schemas/ReviewDrawResponse"})
    );
}

#[test]
fn describes_review_update_route() {
    let listing = build();
    let entry = listing.entries().get(8).unwrap();
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
    let (head, tail) = failing.split_at(6);
    assert_eq!(
        head,
        [
            ("batches", vec![400, 503]),
            ("batches", vec![400, 404, 409, 503]),
            ("health", vec![]),
            ("messages", vec![400, 503]),
            ("prompts", vec![503]),
            ("prompts", vec![400, 404, 422, 503]),
        ]
    );
    assert_eq!(
        tail,
        [
            ("review", vec![400, 409, 503]),
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
