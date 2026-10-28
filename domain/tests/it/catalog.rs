use domain::features::catalog::build;
use domain::features::dispatch;
use serde_json::{Value, json};

use crate::support::{cli, errors};

#[test]
fn lists_health_check_with_schemas() {
    let listing = build();
    assert_eq!(listing.entries().len(), 1);
    let entry = listing.entries().first().unwrap();
    assert_eq!(entry.feature(), "health");
    assert_eq!(entry.action(), "check");
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
        Value::Object(listing.schemas().clone()),
        json!({
            "ErrorBody": {
                "properties": {"code": {"type": "string"}},
                "required": ["code"],
                "type": "object"
            },
            "HealthCheckResponse": {
                "properties": {"status": {"type": "string"}},
                "required": ["status"],
                "type": "object"
            }
        })
    );
}

#[test]
fn unknown_action_of_known_feature_is_rejected() {
    let errors = errors().unwrap();
    let err = dispatch(&errors, &cli("health", "nope", "{}")).unwrap_err();
    assert_eq!(err.status(), 404);
}
