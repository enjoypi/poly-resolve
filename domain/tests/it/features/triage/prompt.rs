use serde_json::{Error, Value, json};

use super::{Reply, call, patched};

const MINUTE: u64 = 60_000;
const NONE: u64 = 0;
const NOON: u64 = 1_783_483_235_000;
const PROBABILITY: f64 = 0.9531;
const ROOMY: u32 = 10_000;

fn brief(rows: &Value, max_chars: u32, template: &str) -> Result<Reply, Error> {
    let request = patched(
        brief_body(rows, template, &Value::Null),
        "max_chars",
        &json!(max_chars),
    );
    call("brief", &request.to_string())
}

fn brief_body(rows: &Value, template: &str, resume: &Value) -> Value {
    json!({
        "followup": "follow:{messages}",
        "max_chars": ROOMY,
        "resume": resume,
        "rows": rows,
        "rules": [],
        "template": template,
    })
}

#[test]
fn brief_drops_oldest_but_keeps_the_newest() {
    let rows = rows();
    let newest = json!([line("alice", "now", "04:00:35")]).to_string();
    let limit = u32::try_from(newest.chars().count()).unwrap();
    let kept = prompt_of(&brief(&rows, limit, "{messages}").unwrap().unwrap());
    assert_eq!(kept, Some(newest));
    let none = prompt_of(&brief(&rows, 0, "x{messages}x").unwrap().unwrap());
    let blank = json!([line("alice", "", "04:00:35")]).to_string();
    assert_eq!(none, Some(format!("x{blank}x")));
}

#[test]
fn brief_keeps_placeholders_in_messages_literal() {
    let rows = json!([{"occurred_at_ms": NOON, "sender": "alice", "text": "{messages}"}]);
    let reply = brief(&rows, ROOMY, "{messages}|{messages}")
        .unwrap()
        .unwrap();
    let expected = json!([line("alice", "{messages}", "04:00:35")]).to_string();
    assert_eq!(prompt_of(&reply), Some(format!("{expected}|{{messages}}")));
}

#[test]
fn brief_masks_again_with_current_rules() {
    let rows = json!([{"occurred_at_ms": NOON, "sender": "alice", "text": "token abc"}]);
    let request = patched(
        brief_body(&rows, "{messages}", &Value::Null),
        "rules",
        &json!([{"pattern": "abc", "placeholder": "[x]"}]),
    );
    let reply = call("brief", &request.to_string()).unwrap().unwrap();
    let expected = json!([line("alice", "token [x]", "04:00:35")]).to_string();
    assert_eq!(prompt_of(&reply), Some(expected));
}

#[test]
fn brief_rejects_invalid_rules() {
    let request = json!({
        "followup": "f",
        "max_chars": ROOMY,
        "rows": [],
        "rules": [{"pattern": "(", "placeholder": "x"}],
        "template": "{messages}",
    })
    .to_string();
    assert_eq!(call("brief", &request).unwrap(), Err(400));
}

#[test]
fn brief_rejects_malformed_request() {
    assert_eq!(call("brief", "nope").unwrap(), Err(400));
    assert_eq!(call("brief", r#"{"rows":[]}"#).unwrap(), Err(400));
}

#[test]
fn brief_resumes_with_only_new_lines() {
    let point = json!({"until": NOON - MINUTE});
    let request = brief_body(&rows(), "ctx:{messages}!", &point);
    let reply = call("brief", &request.to_string()).unwrap().unwrap();
    let expected = json!([line("alice", "now", "04:00:35")]).to_string();
    assert_eq!(prompt_of(&reply), Some(format!("follow:{expected}")));
    assert_eq!(reply.get("start"), Some(&json!("resume")));
    assert_eq!(reply.get("until_ms"), Some(&json!(NOON)));
}

#[test]
fn brief_starts_fresh_with_every_line() {
    let reply = brief(&rows(), ROOMY, "ctx:{messages}!").unwrap().unwrap();
    assert_eq!(reply.get("start"), Some(&json!("fresh")));
    assert_eq!(reply.get("until_ms"), Some(&json!(NOON)));
    let expected = json!([
        line("bob", "ancient", "03:57:35"),
        line("unknown", "before", "03:59:35"),
        line("alice", "now", "04:00:35"),
    ])
    .to_string();
    assert_eq!(prompt_of(&reply), Some(format!("ctx:{expected}!")));
}

#[test]
fn brief_without_rows_reports_zero() {
    let reply = brief(&json!([]), ROOMY, "x{messages}").unwrap().unwrap();
    assert_eq!(reply.get("until_ms"), Some(&json!(NONE)));
    assert_eq!(prompt_of(&reply), Some("x[]".to_owned()));
}

fn line(sender: &str, text: &str, time: &str) -> Value {
    json!({
        "sender": sender,
        "text": text,
        "time_utc": format!("2026-07-08 {time}"),
    })
}

fn notice(analysis: &str, max_chars: u32, template: &str) -> Result<Reply, Error> {
    let request = json!({
        "analysis": analysis,
        "max_chars": max_chars,
        "probability": PROBABILITY,
        "template": template,
        "topic_id": "t1-0",
    })
    .to_string();
    call("notice", &request)
}

#[test]
fn notice_fills_template_and_truncates_analysis() {
    let reply = notice("abcdef", 4, "[{probability}] {topic}\n{analysis}")
        .unwrap()
        .unwrap();
    assert_eq!(reply.get("text"), Some(&json!("[0.95] t1-0\nabcd")));
}

#[test]
fn notice_keeps_placeholders_in_analysis_literal() {
    let reply = notice("see {topic}", ROOMY, "{topic}: {analysis}")
        .unwrap()
        .unwrap();
    assert_eq!(reply.get("text"), Some(&json!("t1-0: see {topic}")));
}

#[test]
fn notice_rejects_malformed_request() {
    assert_eq!(call("notice", "nope").unwrap(), Err(400));
    assert_eq!(call("notice", "{}").unwrap(), Err(400));
}

fn prompt_of(reply: &Value) -> Option<String> {
    reply
        .get("prompt")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn rows() -> Value {
    json!([
        {"occurred_at_ms": NOON, "sender": "alice", "text": "now"},
        {"occurred_at_ms": NOON - 3 * MINUTE, "sender": "bob", "text": "ancient"},
        {"occurred_at_ms": NOON - MINUTE, "sender": "unknown", "text": "before"},
    ])
}
