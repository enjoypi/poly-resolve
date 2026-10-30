use core::error::Error;
use std::fs;
use std::path::Path;

use adapters::store::Archive;
use serde_json::{Value, from_str, json};

use super::{get, start, stop};
use crate::support::{BUSY, EVENT, await_log, config, live, sandbox};

const LIMIT: u32 = 2;

type Fallible<T> = Result<T, Box<dyn Error>>;

fn body(response: &str) -> Fallible<Value> {
    let (_, text) = response
        .split_once("\r\n\r\n")
        .ok_or("response without body")?;
    Ok(from_str(text)?)
}

#[test]
fn collects_events_while_serving() {
    let dir = sandbox("cli-messages-collect").unwrap();
    fs::write(format!("{dir}/listen.txt"), format!("{EVENT}\n")).unwrap();
    let path = config("cli-messages-collect.yaml", &live(&dir)).unwrap();
    let mut server = start(&path).unwrap();
    await_log(&mut server.lines, "event stored").unwrap();
    let response = get(&server.addr, "/api/messages").unwrap();
    assert!(response.starts_with("HTTP/1.1 200"));
    let reply = body(&response).unwrap();
    assert_eq!(keys(&reply), ["dingtalk:acct:m1"]);
    assert!(stop(server).unwrap());
}

fn envelope(key: &str, millis: u64) -> String {
    json!({
        "account": "acct",
        "conversation_id": "c1",
        "key": key,
        "occurred_at_ms": millis,
        "raw": "{}",
        "received_at_ms": millis,
        "sender_id": "u1",
        "sender_name": "alice",
        "source": key.split(':').next().unwrap_or_default(),
        "text": "hello",
    })
    .to_string()
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
fn lists_messages_page_by_page() {
    let path = seeded("cli-messages-list").unwrap();
    let server = start(&path).unwrap();
    let response = get(&server.addr, &format!("/api/messages?limit={LIMIT}")).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"));
    let reply = body(&response).unwrap();
    assert_eq!(keys(&reply), ["dingtalk:acct:m3", "imap:acct:m2"]);
    let cursor = reply.get("next_cursor").and_then(Value::as_str).unwrap();
    let next = format!(
        "/api/messages?cursor={}&limit={LIMIT}",
        cursor.replace(':', "%3A")
    );
    let page = body(&get(&server.addr, &next).unwrap()).unwrap();
    assert_eq!(keys(&page), ["dingtalk:acct:m1"]);
    assert_eq!(page.get("next_cursor"), Some(&Value::Null));
    let bad = get(&server.addr, "/api/messages?limit=0").unwrap();
    assert!(bad.starts_with("HTTP/1.1 400"));
    assert!(stop(server).unwrap());
}

#[test]
fn lists_messages_within_window() {
    let path = seeded("cli-messages-window").unwrap();
    let server = start(&path).unwrap();
    let window = "/api/messages?from_ms=15&to_ms=30";
    let reply = body(&get(&server.addr, window).unwrap()).unwrap();
    assert_eq!(keys(&reply), ["imap:acct:m2"]);
    let inverted = "/api/messages?from_ms=30&to_ms=15";
    let bad = get(&server.addr, inverted).unwrap();
    assert!(bad.starts_with("HTTP/1.1 400"));
    assert!(stop(server).unwrap());
}

fn seeded(name: &str) -> Fallible<String> {
    let dir = sandbox(name)?;
    let store = Archive::open(Path::new(&format!("{dir}/store.db")), BUSY)?;
    for (key, millis) in [
        ("dingtalk:acct:m1", 10),
        ("imap:acct:m2", 20),
        ("dingtalk:acct:m3", 30),
    ] {
        let _: usize = store.save_event(&envelope(key, millis))?;
    }
    Ok(config(&format!("{name}.yaml"), &live(&dir))?)
}

#[test]
fn storage_failure_is_service_unavailable() {
    let dir = sandbox("cli-messages-http").unwrap();
    let path = config("cli-messages-http.yaml", &live(&dir)).unwrap();
    let mut server = start(&path).unwrap();
    await_log(&mut server.lines, "page rejected").unwrap();
    fs::remove_file(format!("{dir}/store.db")).unwrap();
    let response = get(&server.addr, "/api/messages").unwrap();
    assert!(response.starts_with("HTTP/1.1 503"));
    assert_eq!(body(&response).unwrap(), json!({"code": "unavailable"}));
    let health = get(&server.addr, "/api/health").unwrap();
    assert!(health.starts_with("HTTP/1.1 200"));
    assert!(stop(server).unwrap());
}
