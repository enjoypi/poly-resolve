use domain::AppError;
use domain::review::Docket;
use serde_json::{Error, Value, from_str, json};

use crate::support::{Broken, Dock, Fixture, cli, signed};

type Reply = Result<Value, u16>;

#[test]
fn approve_rejects_unusable_requests() {
    assert_eq!(
        rule("approve", r#"{"keys":["nope"]}"#, Some("alice")).unwrap(),
        Err(400)
    );
    assert_eq!(
        rule("approve", r#"{"keys":["k1"]}"#, None).unwrap(),
        Err(400)
    );
    assert_eq!(
        rule("approve", r#"{"keys":["k1"]}"#, Some("")).unwrap(),
        Err(400)
    );
    assert_eq!(rule("approve", "nope", Some("alice")).unwrap(), Err(400));
    assert_eq!(rule("reject", "nope", Some("alice")).unwrap(), Err(400));
    let request = r#"{"key":"k1","reason":"noise"}"#;
    assert_eq!(rule("reject", request, None).unwrap(), Err(400));
}

#[test]
fn approve_settles_pending_items_only() {
    let done = rule("approve", r#"{"keys":["k1","k3","k4"]}"#, Some("alice"))
        .unwrap()
        .unwrap();
    assert_eq!(
        done.get("states"),
        Some(&json!(["approved", "approved", "rejected"]))
    );
}

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

fn list(request: &str, docket: &dyn Docket) -> Result<Reply, Error> {
    reply(Fixture::new()?.dispatch_review(docket, &cli("review", "list", request)))
}

#[test]
fn list_defaults_to_pending_and_returns_reasons() {
    let reply = list("{}", &dock()).unwrap().unwrap();
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
    let approved = list(r#"{"state":"approved"}"#, &dock()).unwrap().unwrap();
    assert_eq!(keys(&approved), ["k2"]);
    let one = list(r#"{"limit":1}"#, &dock()).unwrap().unwrap();
    assert_eq!(keys(&one), ["k1"]);
}

#[test]
fn list_rejects_malformed_request() {
    assert_eq!(list("nope", &dock()).unwrap(), Err(400));
    assert_eq!(list(r#"{"limit":0}"#, &dock()).unwrap(), Err(400));
    assert_eq!(list(r#"{"limit":4}"#, &dock()).unwrap(), Err(400));
}

#[test]
fn list_reports_storage_failure() {
    assert_eq!(list("{}", &Broken).unwrap(), Err(503));
}

#[test]
fn reject_drops_the_line_for_reasons_without_placeholder() {
    let done = rule("reject", r#"{"key":"k1","reason":"noise"}"#, Some("alice"))
        .unwrap()
        .unwrap();
    assert_eq!(done.get("state"), Some(&json!("rejected")));
    let stray = r#"{"key":"k1","leak":"secret-token-123","reason":"noise"}"#;
    assert_eq!(rule("reject", stray, Some("alice")).unwrap(), Err(400));
}

#[test]
fn reject_learns_a_literal_rule_from_the_pasted_leak() {
    let request = r#"{"key":"k1","leak":"  secret-token-123 ","reason":"leak"}"#;
    let done = rule("reject", request, Some("alice")).unwrap().unwrap();
    assert_eq!(
        done.get("state"),
        Some(&json!(r"leaked:secret\-token\-123:[x]"))
    );
}

#[test]
fn reject_refuses_unusable_leaks() {
    for request in [
        r#"{"key":"k1","reason":"leak"}"#,
        r#"{"key":"k1","leak":"  ","reason":"leak"}"#,
        r#"{"key":"k1","leak":"absent-text","reason":"leak"}"#,
        r#"{"key":"k1","leak":"se","reason":"leak"}"#,
        r#"{"key":"nope","leak":"secret-token-123","reason":"leak"}"#,
    ] {
        assert_eq!(rule("reject", request, Some("alice")).unwrap(), Err(400));
    }
}

#[test]
fn reject_requires_a_known_reason() {
    let unknown = rule("reject", r#"{"key":"k1","reason":"other"}"#, Some("alice"));
    assert_eq!(unknown.unwrap(), Err(400));
    assert_eq!(
        rule("reject", r#"{"key":"k1"}"#, Some("alice")).unwrap(),
        Err(400)
    );
}

fn reply(result: Result<String, AppError>) -> Result<Reply, Error> {
    match result {
        Ok(response) => Ok(Ok(from_str(&response)?)),
        Err(err) => Ok(Err(err.status())),
    }
}

fn rule(action: &str, request: &str, user: Option<&str>) -> Result<Reply, Error> {
    reply(Fixture::new()?.dispatch_review(&dock(), &signed(action, request, user)))
}

#[test]
fn settle_and_source_report_storage_failure() {
    for (action, request) in [
        ("approve", r#"{"keys":["k1"]}"#),
        ("reject", r#"{"key":"k1","reason":"noise"}"#),
        (
            "reject",
            r#"{"key":"k1","leak":"secret-token-123","reason":"leak"}"#,
        ),
    ] {
        let result = Fixture::new()
            .unwrap()
            .dispatch_review(&Broken, &signed(action, request, Some("alice")));
        assert_eq!(result.unwrap_err().status(), 503);
    }
}
