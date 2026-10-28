use serde_json::{Error, Value, from_str, json};

use crate::support::{Fixture, cli};

const BODY: &str = "QUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVo0MTIzNDU2";
const CIPHER_MIN: u32 = 32;
const FRAGMENTS: &str = "title\nalice:QUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVo0MTIzNDU2\nQUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVo0MTIzNDU2\nQUJDREVGR0hJSktMTU5PUFFSU1RV";
const LATER: u64 = 1_783_483_240_000;
const LINE: &str = r#"{"type":"user_im_message_receive_o2o_all","event_id":"e1","message_id":"m1","conversation_id":"c1","sender":"alice","sender_open_dingtalk_id":"u1","content":"hello","create_time":"2026-07-08 12:00:35","event_time":1783483235983}"#;
const NOON: u64 = 1_783_483_235_000;
const OFFSET: i16 = 480;
const RECEIVED: u64 = 1_783_483_240_000;
const STAMPED: u64 = 1_783_483_235_983;

type Reply = Result<Value, u16>;

#[test]
fn assemble_rebuilds_text_from_children() {
    let envelope = parse_event(&event_with(FRAGMENTS), OFFSET)
        .unwrap()
        .unwrap();
    let children = json!([
        {"sender": "alice", "text": "first\nsecond"},
        {"sender": "bob", "text": "reply"},
    ]);
    let request = json!({"children": children, "envelope": envelope}).to_string();
    let assembled = call("assemble", &request).unwrap().unwrap();
    assert_eq!(
        text(&assembled, "text"),
        Some("title\nalice:first\nsecond\nbob:reply")
    );
    assert_eq!(assembled.get("lookup"), Some(&Value::Null));
    assert_eq!(text(&assembled, "key"), text(&envelope, "key"));
}

#[test]
fn assemble_rejects_malformed_request() {
    assert_eq!(call("assemble", "nope").unwrap(), Err(400));
    assert_eq!(call("assemble", r#"{"children":[]}"#).unwrap(), Err(400));
}

fn call(action: &str, request: &str) -> Result<Reply, Error> {
    match Fixture::new()?.dispatch(&cli("ingest", action, request)) {
        Ok(response) => Ok(Ok(from_str(&response)?)),
        Err(err) => Ok(Err(err.status())),
    }
}

fn cipher(text: &str, min_length: u32) -> Result<Option<String>, Error> {
    let envelope = json!({
        "account": "acct",
        "conversation_id": "c1",
        "key": "dingtalk:acct:m1",
        "occurred_at_ms": NOON,
        "raw": "{}",
        "received_at_ms": RECEIVED,
        "sender_id": "u1",
        "sender_name": "alice",
        "source": "dingtalk",
        "text": text,
    });
    let request = json!({"envelope": envelope, "min_length": min_length}).to_string();
    Ok(call("cipher", &request)?.ok().and_then(|reply| {
        reply
            .get("cipher")
            .and_then(Value::as_str)
            .map(str::to_owned)
    }))
}

#[test]
fn cipher_accepts_wrapped_base64_with_trailer() {
    for text in [
        format!("{BODY}||2||1||196"),
        format!("{BODY}\n{BODY}+/==\r\n ||2||1||196 \n"),
    ] {
        assert_eq!(cipher(&text, 32).unwrap(), Some(text));
    }
}

#[test]
fn cipher_honours_min_length() {
    let text = "abcde||2||1||9";
    assert_eq!(cipher(text, 5).unwrap(), Some(text.to_owned()));
    assert_eq!(cipher(text, 6).unwrap(), None);
}

#[test]
fn cipher_rejects_malformed_request() {
    assert_eq!(call("cipher", "nope").unwrap(), Err(400));
    assert_eq!(call("cipher", r#"{"min_length":1}"#).unwrap(), Err(400));
}

#[test]
fn cipher_rejects_ordinary_text() {
    for text in [
        "hello".to_owned(),
        "a || b".to_owned(),
        format!("{BODY}||1||196"),
        format!("{BODY}||2||1||19a"),
        format!("{BODY}||2||||196"),
        format!("{BODY}||x||1||196"),
        format!("{BODY}|x||2||1||196"),
        format!("中文{BODY}||2||1||196"),
        "short||2||1||196".to_owned(),
    ] {
        assert_eq!(cipher(&text, 32).unwrap(), None, "{text}");
    }
}

#[test]
fn event_accepts_millis_stamp() {
    let line = r#"{"message_id":"m2","conversation_id":"c1","create_time":1783483235983}"#;
    let envelope = parse_event(line, OFFSET).unwrap().unwrap();
    assert_eq!(number(&envelope, "occurred_at_ms"), Some(STAMPED));
    assert_eq!(text(&envelope, "text"), Some(""));
    assert_eq!(text(&envelope, "sender_id"), Some(""));
    assert_eq!(text(&envelope, "sender_name"), Some(""));
}

#[test]
fn event_accepts_rfc3339_stamp() {
    let line =
        r#"{"message_id":"m3","conversation_id":"c1","create_time":"2026-07-08T12:00:35+08:00"}"#;
    let envelope = parse_event(line, 0).unwrap().unwrap();
    assert_eq!(number(&envelope, "occurred_at_ms"), Some(NOON));
}

#[test]
fn event_before_epoch_is_zero() {
    let line = r#"{"message_id":"m4","conversation_id":"c1","create_time":"1960-01-01 00:00:00"}"#;
    let envelope = parse_event(line, 0).unwrap().unwrap();
    assert_eq!(number(&envelope, "occurred_at_ms"), Some(0));
}

#[test]
fn event_builds_envelope() {
    assert_eq!(
        parse_event(LINE, OFFSET).unwrap().unwrap(),
        json!({
            "account": "acct",
            "conversation_id": "c1",
            "key": "dingtalk:acct:m1",
            "lookup": Value::Null,
            "occurred_at_ms": NOON,
            "raw": LINE,
            "received_at_ms": RECEIVED,
            "sender_id": "u1",
            "sender_name": "alice",
            "source": "dingtalk",
            "text": "hello",
        })
    );
}

#[test]
fn event_flags_sealed_fragments_for_lookup() {
    for text in [
        FRAGMENTS.to_owned(),
        format!("head\n  {BODY}  \ntail"),
        format!("{BODY}+/=="),
    ] {
        let envelope = parse_event(&event_with(&text), OFFSET).unwrap().unwrap();
        assert_eq!(text_of(&envelope, "lookup"), Some("m9"), "{text}");
    }
}

#[test]
fn event_ignores_lines_that_are_not_fragments() {
    for text in [
        "hello".to_owned(),
        "QUJDREVGR0hJSktMTU5PUFFSU1RV".to_owned(),
        "hello world hello world hello world".to_owned(),
        format!("{BODY}-{BODY}"),
        String::new(),
    ] {
        let envelope = parse_event(&event_with(&text), OFFSET).unwrap().unwrap();
        assert_eq!(envelope.get("lookup"), Some(&Value::Null), "{text}");
    }
}

#[test]
fn event_rejects_invalid_input() {
    let missing = r#"{"conversation_id":"c1","create_time":"2026-07-08 12:00:35"}"#;
    let no_stamp = r#"{"message_id":"m7","conversation_id":"c1"}"#;
    let bad_stamp = r#"{"message_id":"m5","conversation_id":"c1","create_time":"yesterday"}"#;
    assert_eq!(parse_event("not json", OFFSET).unwrap(), Err(400));
    assert_eq!(parse_event("[]", OFFSET).unwrap(), Err(400));
    assert_eq!(parse_event(missing, OFFSET).unwrap(), Err(400));
    assert_eq!(parse_event(no_stamp, OFFSET).unwrap(), Err(400));
    assert_eq!(parse_event(bad_stamp, OFFSET).unwrap(), Err(400));
}

#[test]
fn event_rejects_malformed_request() {
    assert_eq!(call("parse_event", "{}").unwrap(), Err(400));
}

fn event_with(text: &str) -> String {
    json!({
        "message_id": "m9",
        "conversation_id": "c1",
        "content": text,
        "create_time": "2026-07-08 12:00:35",
    })
    .to_string()
}

fn number(value: &Value, name: &str) -> Option<u64> {
    value.get(name).and_then(Value::as_u64)
}

#[test]
fn page_accepts_numeric_cursor() {
    let response = parse_page(r#"{"result":{"hasMore":true,"nextCursor":42}}"#)
        .unwrap()
        .unwrap();
    assert_eq!(text(&response, "next_cursor"), Some("42"));
}

#[test]
fn page_builds_envelopes() {
    let body = r#"{"result":{"conversationMessagesList":[{"openConversationId":"c1","messages":[{"messageId":"m1","sender":"alice","senderId":"u1","text":"hi","createTime":"2026-07-08 12:00:35"},{"openMessageId":"m2","senderName":"bob","senderUserId":"u2","content":"yo","createTime":"2026-07-08T12:00:40+08:00"}]},{"openConversationId":"c2","messages":[]}],"hasMore":true,"nextCursor":"abc"}}"#;
    let response = parse_page(body).unwrap().unwrap();
    assert_eq!(text(&response, "next_cursor"), Some("abc"));
    assert_eq!(number(&response, "skipped"), Some(0));
    let envelopes = response.get("envelopes").and_then(Value::as_array).unwrap();
    assert_eq!(envelopes.len(), 2);
    assert_eq!(
        envelopes.first(),
        Some(&json!({
            "account": "acct",
            "conversation_id": "c1",
            "key": "dingtalk:acct:m1",
            "lookup": Value::Null,
            "occurred_at_ms": NOON,
            "raw": r#"{"createTime":"2026-07-08 12:00:35","messageId":"m1","sender":"alice","senderId":"u1","text":"hi"}"#,
            "received_at_ms": RECEIVED,
            "sender_id": "u1",
            "sender_name": "alice",
            "source": "dingtalk",
            "text": "hi",
        }))
    );
    let second = envelopes.last().unwrap();
    assert_eq!(text(second, "key"), Some("dingtalk:acct:m2"));
    assert_eq!(text(second, "conversation_id"), Some("c1"));
    assert_eq!(text(second, "sender_id"), Some("u2"));
    assert_eq!(text(second, "text"), Some("yo"));
    assert_eq!(number(second, "occurred_at_ms"), Some(LATER));
}

#[test]
fn page_flags_only_messages_with_fragments() {
    let page = json!({"result": {"conversationMessagesList": [{
        "openConversationId": "c1",
        "messages": [
            {"messageId": "m1", "content": FRAGMENTS, "createTime": "2026-07-08 12:00:35"},
            {"messageId": "m2", "content": "plain", "createTime": "2026-07-08 12:00:35"},
        ],
    }]}})
    .to_string();
    let reply = parse_page(&page).unwrap().unwrap();
    let lookups: Vec<Option<&str>> = reply
        .get("envelopes")
        .and_then(Value::as_array)
        .map_or_default(|items| items.iter().map(|item| text_of(item, "lookup")).collect());
    assert_eq!(lookups, [Some("m1"), None]);
}

#[test]
fn page_ignores_cursor_when_exhausted() {
    let body = r#"{"result":{"hasMore":false,"nextCursor":"abc"}}"#;
    let response = parse_page(body).unwrap().unwrap();
    assert_eq!(response.get("next_cursor"), Some(&Value::Null));
}

#[test]
fn page_rejects_invalid_input() {
    assert_eq!(parse_page("not json").unwrap(), Err(400));
    assert_eq!(parse_page(r#"{"data":{}}"#).unwrap(), Err(400));
}

#[test]
fn page_rejects_malformed_request() {
    assert_eq!(call("parse_page", "[]").unwrap(), Err(400));
}

#[test]
fn page_skips_malformed_messages() {
    let body = r#"{"result":{"conversationMessagesList":[{"openConversationId":"c1","messages":[{"text":"no id","createTime":"2026-07-08 12:00:35"},{"messageId":"m1","createTime":"yesterday"},{"messageId":"m2","createTime":1783483235983},"not an object"]}]}}"#;
    let response = parse_page(body).unwrap().unwrap();
    assert_eq!(response.get("next_cursor"), Some(&Value::Null));
    assert_eq!(number(&response, "skipped"), Some(3));
    let envelopes = response.get("envelopes").and_then(Value::as_array).unwrap();
    assert_eq!(envelopes.len(), 1);
}

#[test]
fn page_without_groups_is_empty() {
    let response = parse_page(r#"{"result":{}}"#).unwrap().unwrap();
    assert_eq!(response.get("envelopes"), Some(&json!([])));
    assert_eq!(response.get("next_cursor"), Some(&Value::Null));
}

fn parse_event(line: &str, offset: i16) -> Result<Reply, Error> {
    call("parse_event", &request(line, offset))
}

fn parse_page(page: &str) -> Result<Reply, Error> {
    call("parse_page", &request(page, OFFSET))
}

fn request(body: &str, offset: i16) -> String {
    json!({
        "account": "acct",
        "body": body,
        "cipher_min_length": CIPHER_MIN,
        "received_at_ms": RECEIVED,
        "source": "dingtalk",
        "utc_offset_minutes": offset,
    })
    .to_string()
}

#[test]
fn reveal_rejects_malformed_request() {
    assert_eq!(call("reveal", "nope").unwrap(), Err(400));
    assert_eq!(call("reveal", r#"{"plaintext":"x"}"#).unwrap(), Err(400));
}

#[test]
fn reveal_replaces_text_only() {
    let envelope = parse_event(LINE, OFFSET).unwrap().unwrap();
    let request = json!({"envelope": envelope, "plaintext": "clear"}).to_string();
    let revealed = call("reveal", &request).unwrap().unwrap();
    let mut expected = envelope;
    drop(
        expected
            .as_object_mut()
            .unwrap()
            .insert("text".to_owned(), json!("clear")),
    );
    assert_eq!(revealed, expected);
}

fn text<'value>(value: &'value Value, name: &str) -> Option<&'value str> {
    value.get(name).and_then(Value::as_str)
}

fn text_of<'value>(value: &'value Value, name: &str) -> Option<&'value str> {
    value.get(name).and_then(Value::as_str)
}

fn window(from_ms: u64, to_ms: u64) -> Result<Reply, Error> {
    let body = json!({"from_ms": from_ms, "to_ms": to_ms, "utc_offset_minutes": OFFSET});
    call("window", &body.to_string())
}

#[test]
fn window_formats_local_time() {
    assert_eq!(
        window(1_783_483_200_000, 1_783_483_500_000)
            .unwrap()
            .unwrap(),
        json!({"end": "2026-07-08 12:05:00", "start": "2026-07-08 12:00:00"})
    );
}

#[test]
fn window_rejects_invalid_input() {
    assert_eq!(window(u64::MAX, 0).unwrap(), Err(400));
    assert_eq!(window(0, u64::MAX).unwrap(), Err(400));
}

#[test]
fn window_rejects_malformed_request() {
    assert_eq!(call("window", "{}").unwrap(), Err(400));
}
