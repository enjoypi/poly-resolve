use serde_json::{Error, Value, json};

use super::{Reply, call, call_unsealed};

const GROUP: usize = 2;
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
fn redact_hashes_values_into_stable_placeholders() {
    let rules = json!([
        {"pattern": "(?i)https://[a-z./]+", "placeholder": "[link#{hash}]"},
        {"group": GROUP, "pattern": "(pwd=)(\\S+)", "placeholder": "${1}[hidden#{hash}]"},
    ]);
    let text = "https://x.test/a HTTPS://X.test/A https://x.test/b pwd=one pwd=two";
    let reply = redact(text, &rules).unwrap().unwrap();
    let masked = reply.get("text").and_then(Value::as_str).unwrap();
    let parts: Vec<&str> = masked.split(' ').collect();
    assert_eq!(parts.len(), 5);
    assert_eq!(parts.first(), parts.get(1));
    assert_ne!(parts.first(), parts.get(2));
    assert_ne!(parts.get(3), parts.get(4));
    let first = parts.first().copied().unwrap();
    assert!(first.starts_with("[link#") && first.ends_with(']'));
    assert_eq!(first.len(), "[link#]".len() + 8);
    assert!(
        parts
            .get(3)
            .is_some_and(|part| part.starts_with("pwd=[hidden#"))
    );
    let again = redact(masked, &rules).unwrap().unwrap();
    assert_eq!(again.get("text"), Some(&json!(masked)));
    for odd in [
        "pwd=[hidden#zzzzzzzz]",
        "pwd=[hidden#12]",
        "pwd=[hidden#12345678",
    ] {
        let other = redact(odd, &rules).unwrap().unwrap();
        assert_ne!(other.get("text"), Some(&json!(odd)));
    }
}

#[test]
fn redact_keeps_text_without_matches() {
    let reply = redact("plain words", &rules()).unwrap().unwrap();
    assert_eq!(reply.get("text"), Some(&json!("plain words")));
    let none = redact(LONG, &json!([])).unwrap().unwrap();
    assert_eq!(none.get("text"), Some(&json!(LONG)));
}

#[test]
fn redact_rejects_hashed_rules_without_a_key() {
    let rules = json!([{"pattern": "x", "placeholder": "[x#{hash}]"}]);
    let request = json!({"rules": rules, "text": "x"}).to_string();
    assert_eq!(call_unsealed("redact", &request).unwrap(), Err(400));
    let plain = json!({"rules": [], "text": "x"}).to_string();
    assert_eq!(
        call_unsealed("redact", &plain).unwrap().unwrap(),
        json!({"text": "x"})
    );
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
