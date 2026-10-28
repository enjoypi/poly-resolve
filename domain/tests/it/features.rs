mod health;

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

fn parse(capture: &Capture) -> Result<Line, Error> {
    from_str(capture.text().trim())
}
