use serde_json::{Error, Value, json};

use super::{Reply, call};

const LONG: &str = "call 13812345678 now";

fn redact(text: &str, rules: &Value) -> Result<Reply, Error> {
    let request = json!({"rules": rules, "text": text}).to_string();
    call("redact", &request)
}

#[test]
fn redact_applies_rules_in_order() {
    let reply = redact("call 13812345678 password: hunter2", &rules())
        .unwrap()
        .unwrap();
    assert_eq!(
        reply.get("text"),
        Some(&json!("call [num] password: [hidden]"))
    );
}

#[test]
fn redact_keeps_text_without_matches() {
    let reply = redact("plain words", &rules()).unwrap().unwrap();
    assert_eq!(reply.get("text"), Some(&json!("plain words")));
    let none = redact(LONG, &json!([])).unwrap().unwrap();
    assert_eq!(none.get("text"), Some(&json!(LONG)));
}

#[test]
fn redact_rejects_invalid_pattern() {
    let rules = json!([{"pattern": "(", "placeholder": "x"}]);
    assert_eq!(redact(LONG, &rules).unwrap(), Err(400));
}

#[test]
fn redact_rejects_malformed_request() {
    assert_eq!(call("redact", "nope").unwrap(), Err(400));
    assert_eq!(call("redact", r#"{"rules":[]}"#).unwrap(), Err(400));
}

fn rules() -> Value {
    json!([
        {"pattern": "[0-9]{11,}", "placeholder": "[num]"},
        {"pattern": "(?i)(password\\s*[:=]\\s*)\\S+", "placeholder": "${1}[hidden]"},
    ])
}
