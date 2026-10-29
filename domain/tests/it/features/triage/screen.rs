use serde_json::{Error, Value, from_str, json};

use super::{Reply, call};

fn screen(lines: &Value, topics: &Value) -> Result<Reply, Error> {
    let request = json!({
        "instructions": "judge",
        "lines": lines,
        "model": "local",
        "topics": topics,
    });
    call("screen", &request.to_string())
}

#[test]
fn screen_asks_for_a_json_verdict_on_aliased_lines() {
    let lines = json!([
        {"key": "k1", "text": "first"},
        {"key": "k2", "text": "second"},
    ]);
    let topics = json!([{"label": "old", "topic": "t3-0"}]);
    let reply = screen(&lines, &topics).unwrap().unwrap();
    let body: Value = from_str(reply.get("body").and_then(Value::as_str).unwrap()).unwrap();
    assert_eq!(body.get("model"), Some(&json!("local")));
    assert_eq!(
        body.get("response_format"),
        Some(&json!({"type": "json_object"}))
    );
    assert_eq!(
        body.pointer("/messages/0"),
        Some(&json!({"content": "judge", "role": "system"}))
    );
    let user: Value = from_str(
        body.pointer("/messages/1/content")
            .and_then(Value::as_str)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        user,
        json!({
            "lines": [
                {"line": "L1", "text": "first"},
                {"line": "L2", "text": "second"},
            ],
            "topics": [{"label": "old", "topic": "t3-0"}],
        })
    );
    assert!(!body.to_string().contains("\"k1\""));
}

#[test]
fn screen_rejects_malformed_request() {
    assert_eq!(call("screen", "nope").unwrap(), Err(400));
    assert_eq!(call("screen", "{}").unwrap(), Err(400));
}
