use domain::inbox::{Reader, Row};
use serde_json::{Error, Value, from_str, json};

use crate::support::{Broken, Fixture, Rows, local, row};

const FIRST: &str = "10:dingtalk:acct:m1";
const LATEST: u64 = 30;
const SECOND: &str = "20:dingtalk:acct:m2";
const SINGLE: u32 = 1;

type Reply = Result<Value, u16>;

#[test]
fn bad_cursor_is_rejected() {
    let rows = newest_first().unwrap();
    for cursor in ["no-colon", "abc:key", ":key", "-1:key"] {
        let query = format!("cursor={cursor}");
        assert_eq!(call(&rows, &query).unwrap(), Err(400));
    }
}

fn call(reader: &dyn Reader, query: &str) -> Result<Reply, Error> {
    let request = local("messages", "list", "").with_params(query);
    match Fixture::new()?.dispatch_with(reader, &request) {
        Ok(response) => Ok(Ok(from_str(&response)?)),
        Err(err) => Ok(Err(err.status())),
    }
}

#[test]
fn cursor_key_may_contain_colons() {
    let rows = newest_first().unwrap();
    let reply = call(&rows, &format!("cursor={SECOND}")).unwrap().unwrap();
    assert_eq!(keys(&reply), ["dingtalk:acct:m1"]);
}

#[test]
fn empty_store_gives_empty_page() {
    let reply = call(&Rows::default(), "").unwrap().unwrap();
    assert_eq!(reply, json!({"items": [], "next_cursor": null}));
}

#[test]
fn exact_page_has_no_next_cursor() {
    let rows = newest_first().unwrap();
    let reply = call(&rows, "limit=3").unwrap().unwrap();
    assert_eq!(keys(&reply).len(), 3);
    assert_eq!(reply.get("next_cursor"), Some(&Value::Null));
}

#[test]
fn first_page_uses_default_limit() {
    let rows = newest_first().unwrap();
    let reply = call(&rows, "").unwrap().unwrap();
    assert_eq!(keys(&reply), ["dingtalk:acct:m3", "dingtalk:acct:m2"]);
    assert_eq!(reply.get("next_cursor"), Some(&json!(SECOND)));
    assert_eq!(
        reply.pointer("/items/0"),
        Some(&json!({
            "account": "acct",
            "analysis": null,
            "conversation_id": "c1",
            "key": "dingtalk:acct:m3",
            "occurred_at_ms": LATEST,
            "priority": null,
            "received_at_ms": LATEST,
            "sender_id": "u1",
            "sender_name": "alice",
            "source": "dingtalk",
            "text": "hello",
            "topic": null,
        }))
    );
}

#[test]
fn invalid_request_is_rejected() {
    let rows = newest_first().unwrap();
    assert_eq!(call(&rows, "limit=x").unwrap(), Err(400));
    assert_eq!(call(&rows, "from_ms=-1").unwrap(), Err(400));
}

#[test]
fn inverted_window_is_rejected() {
    let rows = newest_first().unwrap();
    for request in ["from_ms=20&to_ms=20", "from_ms=25&to_ms=20"] {
        assert_eq!(call(&rows, request).unwrap(), Err(400));
    }
}

fn keys(reply: &Value) -> Vec<&str> {
    reply
        .get("items")
        .and_then(Value::as_array)
        .map_or_default(|items| {
            items
                .iter()
                .filter_map(|item| item.get("key").and_then(Value::as_str))
                .collect()
        })
}

#[test]
fn limit_must_be_within_bounds() {
    let rows = newest_first().unwrap();
    assert_eq!(call(&rows, "limit=0").unwrap(), Err(400));
    assert_eq!(call(&rows, "limit=4").unwrap(), Err(400));
    let reply = call(&rows, "limit=1").unwrap().unwrap();
    assert_eq!(keys(&reply), ["dingtalk:acct:m3"]);
}

fn newest_first() -> Result<Rows, Error> {
    let rows: Vec<Row> = vec![
        row("dingtalk:acct:m3", 30)?,
        row("dingtalk:acct:m2", 20)?,
        row("dingtalk:acct:m1", 10)?,
    ];
    Ok(Rows::new(rows))
}

#[test]
fn next_page_follows_cursor() {
    let rows = newest_first().unwrap();
    let query = format!("cursor={SECOND}&limit={SINGLE}");
    let reply = call(&rows, &query).unwrap().unwrap();
    assert_eq!(keys(&reply), ["dingtalk:acct:m1"]);
    assert_eq!(reply.get("next_cursor"), Some(&Value::Null));
    let last = call(&rows, &format!("cursor={FIRST}")).unwrap().unwrap();
    assert_eq!(keys(&last), Vec::<&str>::new());
}

#[test]
fn storage_failure_is_reported() {
    assert_eq!(call(&Broken, "").unwrap(), Err(503));
}

#[test]
fn window_limits_rows_to_half_open_range() {
    let rows = newest_first().unwrap();
    let cases = [
        ("from_ms=20", vec!["dingtalk:acct:m3", "dingtalk:acct:m2"]),
        ("to_ms=30", vec!["dingtalk:acct:m2", "dingtalk:acct:m1"]),
        ("from_ms=15&to_ms=25", vec!["dingtalk:acct:m2"]),
        ("from_ms=40", vec![]),
    ];
    for (request, expected) in cases {
        let reply = call(&rows, request).unwrap().unwrap();
        assert_eq!(keys(&reply), expected);
    }
}
