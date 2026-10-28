use serde_json::{Error, Value, from_str, json};

use crate::support::{Fixture, cli};

const BODY: &str = "QUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVo0MTIzNDU2";
const CIPHER_MIN: u32 = 32;
const HIGH: f64 = 0.95;
const INPUT_MAX: u32 = 8;
const LOW: f64 = 0.79;
const MID: f64 = 0.9;
const MINUTE: u64 = 60_000;
const MULTIBYTE: &str =
    "\u{670d}\u{52a1}\u{5668}\u{5b95}\u{673a}\u{4e86}\u{8bf7}\u{9a6c}\u{4e0a}\u{5904}\u{7406}";
const MULTIBYTE_HEAD: &str = "\u{670d}\u{52a1}\u{5668}\u{5b95}\u{673a}\u{4e86}\u{8bf7}\u{9a6c}";
const NOON: u64 = 1_783_483_235_000;
const NOTICE: &str = "notice-conversation";
const OWN: &str = "me";
const PROBABILITY: f64 = 0.9531;
const ROOMY: u32 = 10_000;
const THRESHOLD: f64 = 0.8;
const WINDOW: u64 = 2 * MINUTE;

type Reply = Result<Value, u16>;

fn ask(sender: &str, conversation: &str, text: &str) -> Result<Value, Error> {
    let request = json!({
        "cipher_min_length": CIPHER_MIN,
        "input_max_chars": INPUT_MAX,
        "message": {"conversation_id": conversation, "sender_id": sender, "text": text},
        "model": "m",
        "notify_conversation_id": NOTICE,
        "question": {"instructions": "is it urgent", "name": "urgent"},
        "self_sender_id": OWN,
    })
    .to_string();
    Ok(call("ask", &request)?.unwrap_or_default())
}

#[test]
fn ask_builds_decision_body() {
    let reply = ask("u1", "c1", "server down now").unwrap();
    assert_eq!(reply.get("skip"), Some(&Value::Null));
    let body: Value = from_str(reply.get("body").and_then(Value::as_str).unwrap()).unwrap();
    assert_eq!(
        body,
        json!({
            "input": "server d",
            "model": "m",
            "questions": [{"instructions": "is it urgent", "name": "urgent", "type": "predicate"}],
        })
    );
}

#[test]
fn ask_rejects_malformed_request() {
    assert_eq!(call("ask", "nope").unwrap(), Err(400));
    assert_eq!(call("ask", "{}").unwrap(), Err(400));
}

#[test]
fn ask_skips_each_reason() {
    let sealed = format!("{BODY}||2||1||196");
    for (sender, conversation, text, reason) in [
        (OWN, "c1", "hello", "own"),
        ("u1", NOTICE, "hello", "notice"),
        ("u1", "c1", " \n ", "empty"),
        ("u1", "c1", sealed.as_str(), "sealed"),
    ] {
        let reply = ask(sender, conversation, text).unwrap();
        assert_eq!(reply.get("body"), Some(&Value::Null), "{reason}");
        assert_eq!(reply.get("skip"), Some(&json!(reason)));
    }
}

#[test]
fn ask_truncates_by_characters() {
    let reply = ask("u1", "c1", MULTIBYTE).unwrap();
    let body: Value = from_str(reply.get("body").and_then(Value::as_str).unwrap()).unwrap();
    assert_eq!(body.get("input"), Some(&json!(MULTIBYTE_HEAD)));
}

fn brief(rows: &Value, max_chars: u32, template: &str) -> Result<Reply, Error> {
    let request = json!({
        "max_chars": max_chars,
        "rows": rows,
        "template": template,
        "trigger_ms": NOON,
        "window_ms": WINDOW,
    })
    .to_string();
    call("brief", &request)
}

#[test]
fn brief_drops_oldest_beyond_limit() {
    let rows = rows();
    let newest = json!([line("alice", "now", "04:00:35")]).to_string();
    let limit = u32::try_from(newest.chars().count()).unwrap();
    let kept = prompt_of(&brief(&rows, limit, "{messages}").unwrap().unwrap());
    assert_eq!(kept, Some(newest));
    let none = prompt_of(&brief(&rows, 0, "x{messages}x").unwrap().unwrap());
    assert_eq!(none.as_deref(), Some("x[]x"));
}

#[test]
fn brief_keeps_placeholders_in_messages_literal() {
    let rows = json!([{
        "occurred_at_ms": NOON,
        "sender_id": "u2",
        "sender_name": "alice",
        "text": "{messages}",
    }]);
    let reply = brief(&rows, ROOMY, "{messages}|{messages}")
        .unwrap()
        .unwrap();
    let expected = json!([line("alice", "{messages}", "04:00:35")]).to_string();
    assert_eq!(prompt_of(&reply), Some(format!("{expected}|{{messages}}")));
}

#[test]
fn brief_rejects_malformed_request() {
    assert_eq!(call("brief", "nope").unwrap(), Err(400));
    assert_eq!(call("brief", r#"{"rows":[]}"#).unwrap(), Err(400));
}

#[test]
fn brief_selects_window_and_orders_oldest_first() {
    let reply = brief(&rows(), ROOMY, "ctx:{messages}!").unwrap().unwrap();
    let expected = json!([
        line("u3", "before", "03:59:35"),
        line("alice", "now", "04:00:35"),
    ])
    .to_string();
    assert_eq!(prompt_of(&reply), Some(format!("ctx:{expected}!")));
}

fn call(action: &str, request: &str) -> Result<Reply, Error> {
    match Fixture::new()?.dispatch(&cli("triage", action, request)) {
        Ok(response) => Ok(Ok(from_str(&response)?)),
        Err(err) => Ok(Err(err.status())),
    }
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
        "conversation_id": "c1",
        "max_chars": max_chars,
        "probability": PROBABILITY,
        "template": template,
    })
    .to_string();
    call("notice", &request)
}

#[test]
fn notice_fills_template_and_truncates_analysis() {
    let reply = notice("abcdef", 4, "[{probability}] {conversation}\n{analysis}")
        .unwrap()
        .unwrap();
    assert_eq!(reply.get("text"), Some(&json!("[0.95] c1\nabcd")));
}

#[test]
fn notice_keeps_placeholders_in_analysis_literal() {
    let reply = notice("see {conversation}", ROOMY, "{conversation}: {analysis}")
        .unwrap()
        .unwrap();
    assert_eq!(reply.get("text"), Some(&json!("c1: see {conversation}")));
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
        {"occurred_at_ms": NOON + MINUTE, "sender_id": "u2", "sender_name": "alice", "text": "later"},
        {"occurred_at_ms": NOON, "sender_id": "u2", "sender_name": "alice", "text": "now"},
        {"occurred_at_ms": NOON - 3 * MINUTE, "sender_id": "u3", "sender_name": "bob", "text": "ancient"},
        {"occurred_at_ms": NOON - MINUTE, "sender_id": "u3", "sender_name": "", "text": "before"},
    ])
}

fn score(body: &str) -> Result<Reply, Error> {
    let request = json!({"body": body, "name": "urgent", "threshold": THRESHOLD}).to_string();
    call("score", &request)
}

#[test]
fn score_ignores_other_answers() {
    let body = r#"{"answers":[{"type":"predicate","name":"other","probability":0.1},{"type":"predicate","name":"urgent","probability":0.9}]}"#;
    let reply = score(body).unwrap().unwrap();
    assert_eq!(reply.get("probability"), Some(&json!(MID)));
    assert_eq!(reply.get("verdict"), Some(&json!("urgent")));
}

#[test]
fn score_judges_against_threshold() {
    for (probability, verdict) in [(HIGH, "urgent"), (THRESHOLD, "urgent"), (LOW, "normal")] {
        let body = json!({"answers": [{"name": "urgent", "probability": probability}]});
        let reply = score(&body.to_string()).unwrap().unwrap();
        assert_eq!(reply.get("probability"), Some(&json!(probability)));
        assert_eq!(reply.get("verdict"), Some(&json!(verdict)));
    }
}

#[test]
fn score_rejects_malformed_request() {
    assert_eq!(call("score", "nope").unwrap(), Err(400));
    assert_eq!(call("score", r#"{"body":"{}"}"#).unwrap(), Err(400));
}

#[test]
fn score_rejects_unusable_replies() {
    for body in [
        "nope",
        "{}",
        r#"{"answers":[]}"#,
        r#"{"answers":[{"name":"other","probability":0.9}]}"#,
        r#"{"answers":[{"name":"urgent"}]}"#,
        r#"{"answers":[{"name":"urgent","probability":null}]}"#,
        r#"{"answers":[{"name":"urgent","probability":1.5}]}"#,
        r#"{"answers":[{"name":"urgent","probability":-0.1}]}"#,
    ] {
        assert_eq!(score(body).unwrap(), Err(400), "{body}");
    }
}
