use serde_json::{Error, Value, json};

use super::{Reply, call};

const MIN_CHARS: u32 = 4;
const SECRET: &str = "abc-secret-9";

fn completion(content: &Value) -> String {
    json!({"choices": [{"message": {"content": content.to_string()}}]}).to_string()
}

fn lines() -> Value {
    json!([
        {"key": "k1", "text": format!("token {SECRET} here")},
        {"key": "k2", "text": "plain words"},
        {"key": "k3", "text": "more words"},
    ])
}

fn screened(response: &str) -> Result<Reply, Error> {
    let request = json!({
        "lines": lines(),
        "min_chars": MIN_CHARS,
        "placeholder": "[x]",
        "response": response,
        "topics": [{"label": "old", "topic": "t3-0"}, {"label": "other", "topic": "t4-0"}],
    });
    call("screened", &request.to_string())
}

#[test]
fn screened_flags_risky_lines_without_usable_spans_and_missing_verdicts() {
    let content = json!({"lines": [
        verdict("L1", "high", [&json!(["absent-text"]), &json!([])]),
        verdict("L2", "high", [&json!([]), &json!([])]),
    ]});
    let reply = screened(&completion(&content)).unwrap().unwrap();
    let flags: Vec<&str> = reply
        .get("lines")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .filter_map(|line| line.get("flag").and_then(Value::as_str))
        .collect();
    assert_eq!(flags, ["unmasked", "unmasked", "missing"]);
    let texts: Vec<&str> = reply
        .get("lines")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .filter_map(|line| line.get("text").and_then(Value::as_str))
        .collect();
    assert_eq!(texts.first(), Some(&"token abc-secret-9 here"));
}

#[test]
fn screened_masks_valid_spans_and_keeps_known_hints_once() {
    let content = json!({"lines": [
        verdict("L1", "high", [&json!([SECRET, "  ", "no", "absent-text"]), &json!(["t3-0", "zzz", "t3-0"])]),
        verdict("L2", "low", [&json!([]), &json!(["t4-0"])]),
        verdict("L3", "low", [&json!([]), &json!([])]),
    ]});
    let reply = screened(&completion(&content)).unwrap().unwrap();
    assert_eq!(
        reply.get("lines"),
        Some(&json!([
            {"flag": "", "hints": ["t3-0"], "key": "k1", "text": "token [x] here"},
            {"flag": "", "hints": ["t4-0"], "key": "k2", "text": "plain words"},
            {"flag": "", "hints": [], "key": "k3", "text": "more words"},
        ]))
    );
}

#[test]
fn screened_rejects_malformed_request() {
    assert_eq!(call("screened", "nope").unwrap(), Err(400));
    assert_eq!(call("screened", "{}").unwrap(), Err(400));
}

#[test]
fn screened_rejects_unusable_responses() {
    for response in [
        "nope".to_owned(),
        "{}".to_owned(),
        r#"{"choices":[]}"#.to_owned(),
        r#"{"choices":[{"message":{"content":"nope"}}]}"#.to_owned(),
        completion(&json!({"rows": []})),
    ] {
        assert_eq!(screened(&response).unwrap(), Err(400), "{response}");
    }
}

fn verdict(line: &str, risk: &str, found: [&Value; 2]) -> Value {
    let [spans, topics] = found;
    json!({"line": line, "risk": risk, "spans": spans, "topics": topics})
}
