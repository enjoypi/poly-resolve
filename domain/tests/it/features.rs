mod health;
mod ingest;

use domain::features::{Call, Origin, Target, dispatch};
use serde::Deserialize;
use serde_json::{Error, Map, Value, from_str, json};

use crate::log::Capture;
use crate::support::{cli, errors};

#[derive(Deserialize)]
struct Line {
    fields: Map<String, Value>,
    level: String,
    target: String,
}

#[test]
fn dispatch_logs_all_fields() {
    let capture = Capture::default();
    let call = Call::new(
        Origin::new("http", Some("s1"), Some("u1")),
        Target::new("health", "check"),
        "{}",
    );
    let body = capture
        .record(|| dispatch(&errors().unwrap(), &call))
        .unwrap();
    let mut line = parse(&capture).unwrap();
    let elapsed_us = line
        .fields
        .remove("elapsed_us")
        .as_ref()
        .and_then(Value::as_u64)
        .unwrap();
    assert_eq!(line.target, "dispatch");
    assert_eq!(line.level, "DEBUG");
    assert!(elapsed_us < 1_000_000);
    assert_eq!(
        Value::Object(line.fields),
        json!({
            "action": "check",
            "feature": "health",
            "ok": true,
            "request": "{}",
            "response": body,
            "session_id": "s1",
            "transport": "http",
            "user": "u1",
        })
    );
}

#[test]
fn dispatch_logs_error_code_as_response() {
    let errors = errors().unwrap();
    let capture = Capture::default();
    let err = capture
        .record(|| dispatch(&errors, &cli("nope", "nope", "{}")))
        .unwrap_err();
    let line = parse(&capture).unwrap();
    assert_eq!(line.fields.get("ok"), Some(&json!(false)));
    assert_eq!(line.fields.get("session_id"), Some(&json!("")));
    assert_eq!(line.fields.get("user"), Some(&json!("")));
    assert_eq!(line.fields.get("response"), Some(&json!(err.body(&errors))));
}

#[test]
fn dispatch_redacts_ingest_errors() {
    let errors = errors().unwrap();
    let capture = Capture::default();
    let call = cli("ingest", "parse_event", "top-secret-body");
    let err = capture.record(|| dispatch(&errors, &call)).unwrap_err();
    let line = parse(&capture).unwrap();
    assert!(!capture.text().contains("top-secret-body"));
    assert_eq!(
        line.fields.get("response"),
        Some(&json!(format!("redacted:{}", err.body(&errors).len())))
    );
    assert_eq!(line.fields.get("ok"), Some(&json!(false)));
}

#[test]
fn dispatch_redacts_ingest_payloads() {
    let capture = Capture::default();
    let (offset, received): (i16, u64) = (480, 1);
    let request = json!({
        "account": "acct",
        "body": r#"{"message_id":"m1","conversation_id":"c1","content":"top-secret-body","create_time":"2026-07-08 12:00:35"}"#,
        "received_at_ms": received,
        "source": "dingtalk",
        "utc_offset_minutes": offset,
    })
    .to_string();
    let call = cli("ingest", "parse_event", &request);
    let body = capture
        .record(|| dispatch(&errors().unwrap(), &call))
        .unwrap();
    let line = parse(&capture).unwrap();
    assert!(!capture.text().contains("top-secret-body"));
    assert_eq!(
        line.fields.get("request"),
        Some(&json!(format!("redacted:{}", request.len())))
    );
    assert_eq!(
        line.fields.get("response"),
        Some(&json!(format!("redacted:{}", body.len())))
    );
    assert_eq!(line.fields.get("ok"), Some(&json!(true)));
}

fn parse(capture: &Capture) -> Result<Line, Error> {
    from_str(capture.text().trim())
}
