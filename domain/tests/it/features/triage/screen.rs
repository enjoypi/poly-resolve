use serde_json::{Error, Value, from_str, json};

use super::{Reply, call};

const MAX_TOKENS: u32 = 64;

fn screen(lines: &Value, topics: &Value) -> Result<Reply, Error> {
    let request = json!({
        "instructions": "judge",
        "lines": lines,
        "max_tokens": MAX_TOKENS,
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
    assert_eq!(body.get("max_tokens"), Some(&json!(MAX_TOKENS)));
    assert_eq!(
        body.get("response_format"),
        Some(&json!({"type": "json_object"}))
    );
    assert_eq!(
        body.pointer("/messages/0"),
        Some(&json!({"content": "judge", "role": "system"}))
    );
    let known: Value = from_str(
        body.pointer("/messages/1/content")
            .and_then(Value::as_str)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        known,
        json!({"topics": [{"label": "old", "topic": "t3-0"}]})
    );
    let shown: Value = from_str(
        body.pointer("/messages/2/content")
            .and_then(Value::as_str)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        shown,
        json!({"lines": [
            {"line": "L1", "text": "first"},
            {"line": "L2", "text": "second"},
        ]})
    );
    assert_eq!(body.pointer("/messages/2/role"), Some(&json!("user")));
    assert!(!body.to_string().contains("\"k1\""));
}

#[test]
fn screen_rejects_malformed_request() {
    assert_eq!(call("screen", "nope").unwrap(), Err(400));
    assert_eq!(call("screen", "{}").unwrap(), Err(400));
}
