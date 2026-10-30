use domain::AppError;
use domain::review::Docket;
use serde_json::{Error, Value, from_str, json};

use crate::support::{Broken, Dock, Fixture, local};

type Reply = Result<Value, u16>;

fn dock() -> Dock {
    Dock::new(&[
        ("k1", "pending"),
        ("k2", "approved"),
        ("k3", "pending"),
        ("k4", "rejected"),
    ])
}

fn keys(reply: &Value) -> Vec<&str> {
    reply
        .get("items")
        .and_then(Value::as_array)
        .map_or_default(|items| {
            items
                .iter()
                .filter_map(|item| item.get("key").and_then(Value::as_str))
                .collect()
        })
}

fn list(query: &str, docket: &dyn Docket) -> Result<Reply, Error> {
    let call = local("review", "list", "").with_params(query);
    reply(Fixture::new()?.dispatch_review(docket, &call))
}

#[test]
fn list_defaults_to_pending_and_returns_reasons() {
    let reply = list("", &dock()).unwrap().unwrap();
    assert_eq!(keys(&reply), ["k1", "k3"]);
    let reasons = reply.get("reasons").and_then(Value::as_array).unwrap();
    assert_eq!(reasons.len(), 2);
    assert_eq!(
        reasons.first().and_then(|reason| reason.get("placeholder")),
        Some(&json!("[x]"))
    );
}

#[test]
fn list_filters_by_state_and_limit() {
    let approved = list("state=approved", &dock()).unwrap().unwrap();
    assert_eq!(keys(&approved), ["k2"]);
    let one = list("limit=1", &dock()).unwrap().unwrap();
    assert_eq!(keys(&one), ["k1"]);
}

#[test]
fn list_rejects_malformed_request() {
    assert_eq!(list("limit=x", &dock()).unwrap(), Err(400));
    assert_eq!(list("limit=0", &dock()).unwrap(), Err(400));
    assert_eq!(list("limit=4", &dock()).unwrap(), Err(400));
}

#[test]
fn list_reports_storage_failure() {
    assert_eq!(list("", &Broken).unwrap(), Err(503));
}

fn reply(result: Result<String, AppError>) -> Result<Reply, Error> {
    match result {
        Ok(response) => Ok(Ok(from_str(&response)?)),
        Err(err) => Ok(Err(err.status())),
    }
}

fn update(key: &str, body: &str) -> Result<Reply, Error> {
    update_on(&dock(), key, body)
}

#[test]
fn update_approves_pending_and_repeats_idempotently() {
    let approved = update("k1", r#"{"state":"approved"}"#).unwrap().unwrap();
    assert_eq!(approved, json!({"state": "approved"}));
    let again = update("k2", r#"{"state":"approved"}"#).unwrap().unwrap();
    assert_eq!(again, json!({"state": "approved"}));
}

#[test]
fn update_conflicts_with_a_different_settled_state() {
    assert_eq!(update("k4", r#"{"state":"approved"}"#).unwrap(), Err(409));
    let reject = r#"{"state":"rejected","reason":"noise"}"#;
    assert_eq!(update("k2", reject).unwrap(), Err(409));
}

#[test]
fn update_drops_the_line_for_reasons_without_placeholder() {
    let done = update("k1", r#"{"state":"rejected","reason":"noise"}"#)
        .unwrap()
        .unwrap();
    assert_eq!(done, json!({"state": "rejected"}));
    let stray = r#"{"state":"rejected","leak":"secret-token-123","reason":"noise"}"#;
    assert_eq!(update("k1", stray).unwrap(), Err(422));
}

#[test]
fn update_learns_a_literal_rule_from_the_pasted_leak() {
    let body = r#"{"state":"rejected","leak":"  secret-token-123 ","reason":"leak"}"#;
    let done = update("k1", body).unwrap().unwrap();
    assert_eq!(done, json!({"state": "leaked"}));
    let wrong = Dock::new(&[("k1", "pending")]);
    let pasted = r#"{"state":"rejected","leak":"secret-token","reason":"leak"}"#;
    assert_eq!(update_on(&wrong, "k1", pasted).unwrap(), Err(409));
}

fn update_on(docket: &dyn Docket, key: &str, body: &str) -> Result<Reply, Error> {
    let query = format!("key={key}");
    let call = local("review", "update", body).with_params(&query);
    reply(Fixture::new()?.dispatch_review(docket, &call))
}

#[test]
fn update_refuses_unusable_leaks() {
    for body in [
        r#"{"state":"rejected","reason":"leak"}"#,
        r#"{"state":"rejected","leak":"  ","reason":"leak"}"#,
        r#"{"state":"rejected","leak":"absent-text","reason":"leak"}"#,
        r#"{"state":"rejected","leak":"se","reason":"leak"}"#,
        r#"{"state":"rejected","reason":"other"}"#,
    ] {
        assert_eq!(update("k1", body).unwrap(), Err(422));
    }
}

#[test]
fn update_rejects_malformed_requests() {
    assert_eq!(update("k1", "nope").unwrap(), Err(400));
    assert_eq!(update("k1", r#"{"state":"maybe"}"#).unwrap(), Err(400));
    assert_eq!(update("k1", r#"{"state":"rejected"}"#).unwrap(), Err(400));
    let call = local("review", "update", r#"{"state":"approved"}"#);
    let missing = reply(Fixture::new().unwrap().dispatch_review(&dock(), &call));
    assert_eq!(missing.unwrap(), Err(400));
}

#[test]
fn update_reports_missing_items() {
    assert_eq!(update("nope", r#"{"state":"approved"}"#).unwrap(), Err(404));
    let leak = r#"{"state":"rejected","leak":"secret-token-123","reason":"leak"}"#;
    assert_eq!(update("nope", leak).unwrap(), Err(404));
}

#[test]
fn update_reports_storage_failure() {
    for body in [
        r#"{"state":"approved"}"#,
        r#"{"state":"rejected","reason":"noise"}"#,
        r#"{"state":"rejected","leak":"secret-token-123","reason":"leak"}"#,
    ] {
        assert_eq!(update_on(&Broken, "k1", body).unwrap(), Err(503));
    }
}
