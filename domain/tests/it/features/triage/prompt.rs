use serde_json::{Error, Value, json};

use super::{Reply, call, patched};

const CONVERSATIONS: u64 = 2;
const COPIES: u64 = 2;
const MINUTE: u64 = 60_000;
const NOON: u64 = 1_783_483_235_000;
const PEOPLE: u64 = 3;
const ROOMY: u32 = 10_000;
const SCORE: f64 = 0.9531;
const SOURCES: u64 = 1;
const URGENCY: f64 = 0.8;

fn brief(rows: &Value, max_chars: u32, template: &str) -> Result<Reply, Error> {
    let request = patched(
        brief_body(rows, template, &Value::Null),
        "max_chars",
        &json!(max_chars),
    );
    call("brief", &request.to_string())
}

fn brief_body(rows: &Value, template: &str, thread: &Value) -> Value {
    json!({
        "followup": "follow:{messages}",
        "max_chars": ROOMY,
        "omitted": "{count} held: {reason}",
        "rows": rows,
        "rules": [],
        "template": template,
        "thread": thread,
    })
}

#[test]
fn brief_keeps_one_copy_of_repeated_rows() {
    let rows = json!([
        {"digest": "d1", "occurred_at_ms": NOON, "sender": "alice", "text": "fwd"},
        {"digest": "d1", "occurred_at_ms": NOON + MINUTE, "sender": "bob", "text": "fwd"},
        {"digest": "d1", "occurred_at_ms": NOON + MINUTE, "omitted": "sealed", "sender": "", "text": ""},
        {"digest": "d2", "occurred_at_ms": NOON + 2 * MINUTE, "sender": "carol", "text": "one"},
        {"occurred_at_ms": NOON + 2 * MINUTE, "sender": "carol", "text": "two"},
    ]);
    let reply = brief(&rows, ROOMY, "{messages}").unwrap().unwrap();
    let expected = json!([
        {"copies": COPIES, "sender": "alice", "text": "fwd", "time_utc": "2026-07-08 04:00:35"},
        {"note": "1 held: sealed"},
        line("carol", "one", "04:02:35"),
        line("carol", "two", "04:02:35"),
    ])
    .to_string();
    assert_eq!(prompt_of(&reply), Some(expected));
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
fn brief_marks_the_source_and_cuts_long_rows() {
    let rows = json!([
        {"occurred_at_ms": NOON, "sender": "alice", "source": "imap", "text": "abcdef"},
    ]);
    let reply = brief(&rows, 3, "{messages}").unwrap().unwrap();
    let expected = json!([{
        "sender": "alice",
        "source": "imap",
        "text": "abc",
        "time_utc": "2026-07-08 04:00:35",
    }])
    .to_string();
    assert_eq!(prompt_of(&reply), Some(expected));
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
fn brief_notes_runs_of_withheld_rows() {
    let held = |back: u64, reason: &str| json!({"occurred_at_ms": NOON - back, "omitted": reason, "sender": "", "text": ""});
    let rows = json!([
        held(3 * MINUTE, "sealed"),
        held(2 * MINUTE, "sealed"),
        held(MINUTE, "empty"),
        {"occurred_at_ms": NOON, "sender": "alice", "text": "now"},
        {"occurred_at_ms": NOON + MINUTE, "omitted": "sealed", "sender": "", "text": ""},
    ]);
    let reply = brief(&rows, ROOMY, "{messages}").unwrap().unwrap();
    let expected = json!([
        {"note": "2 held: sealed"},
        {"note": "1 held: empty"},
        line("alice", "now", "04:00:35"),
        {"note": "1 held: sealed"},
    ])
    .to_string();
    assert_eq!(prompt_of(&reply), Some(expected));
}

#[test]
fn brief_rejects_invalid_rules() {
    let request = json!({
        "followup": "f",
        "max_chars": ROOMY,
        "omitted": "",
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
fn brief_resumes_with_every_line_again() {
    let request = brief_body(&rows(), "ctx:{messages}!", &json!("T1"));
    let reply = call("brief", &request.to_string()).unwrap().unwrap();
    assert_eq!(prompt_of(&reply), Some(format!("follow:{}", whole())));
    assert_eq!(reply.get("start"), Some(&json!("resume")));
    let blank = brief_body(&rows(), "ctx:{messages}!", &json!(""));
    let fresh = call("brief", &blank.to_string()).unwrap().unwrap();
    assert_eq!(fresh.get("start"), Some(&json!("fresh")));
}

#[test]
fn brief_starts_fresh_with_every_line() {
    let reply = brief(&rows(), ROOMY, "ctx:{messages}!").unwrap().unwrap();
    assert_eq!(reply.get("start"), Some(&json!("fresh")));
    assert_eq!(prompt_of(&reply), Some(format!("ctx:{}!", whole())));
}

#[test]
fn brief_without_rows_is_empty() {
    let reply = brief(&json!([]), ROOMY, "x{messages}").unwrap().unwrap();
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
        "changes": "moved",
        "label": "server d",
        "late": "(at {time})",
        "max_chars": max_chars,
        "reach": {"conversations": CONVERSATIONS, "people": PEOPLE, "sources": SOURCES},
        "score": SCORE,
        "state": "resolved",
        "states": {"resolved": "done"},
        "template": template,
        "topic_id": "t1-0",
        "urgency": URGENCY,
    });
    call("notice", &request.to_string())
}

#[test]
fn notice_fills_template_and_truncates_analysis() {
    let template = "[{score}={urgency}+{people}/{conversations}/{sources}] {state} {label} {topic} {changes}{late}\n{analysis}";
    let reply = notice("abcdef", 4, template).unwrap().unwrap();
    assert_eq!(
        reply.get("text"),
        Some(&json!("[0.95=0.80+3/2/1] done server d t1-0 moved\nabcd"))
    );
}

#[test]
fn notice_keeps_placeholders_in_analysis_literal() {
    let reply = notice("see {topic}", ROOMY, "{topic}: {analysis}")
        .unwrap()
        .unwrap();
    assert_eq!(reply.get("text"), Some(&json!("t1-0: see {topic}")));
}

#[test]
fn notice_marks_late_messages_and_tolerates_missing_parts() {
    let request = json!({
        "analysis": "a",
        "label": "l",
        "late": "(at {time})",
        "late_ms": NOON,
        "max_chars": ROOMY,
        "state": "open",
        "template": "{state}|{score}|{people}|{late}|{analysis}",
        "topic_id": "t",
    });
    let reply = call("notice", &request.to_string()).unwrap().unwrap();
    assert_eq!(
        reply.get("text"),
        Some(&json!("open||0|(at 2026-07-08 04:00:35 UTC)|a"))
    );
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

fn whole() -> String {
    json!([
        line("bob", "ancient", "03:57:35"),
        line("unknown", "before", "03:59:35"),
        line("alice", "now", "04:00:35"),
    ])
    .to_string()
}
