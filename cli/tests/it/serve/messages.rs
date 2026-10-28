use core::error::Error;
use std::fs;
use std::path::Path;
use std::process::{Command, ExitCode};

use adapters::store::Archive;
use serde_json::{Value, from_slice, from_str, json};

use super::{post, start, stop};
use crate::support::{EVENT, YAML, await_log, call, config, live, sandbox};

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
    let response = post(&server.addr, ["/messages/list", ""], "{}").unwrap();
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
        "source": "dingtalk",
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
fn lists_messages_identically_on_both_channels() {
    let path = seeded("cli-messages-list").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_cli"))
        .args(["call", "messages", "list", "--json", r#"{"limit":2}"#])
        .args(["--config", &path])
        .output()
        .unwrap();
    assert!(out.status.success());
    let cli_reply: Value = from_slice(&out.stdout).unwrap();
    let server = start(&path).unwrap();
    let response = post(&server.addr, ["/messages/list", ""], r#"{"limit":2}"#).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"));
    let http_reply = body(&response).unwrap();
    assert_eq!(cli_reply, http_reply);
    assert_eq!(keys(&http_reply), ["dingtalk:acct:m3", "dingtalk:acct:m2"]);
    let cursor = http_reply
        .get("next_cursor")
        .and_then(Value::as_str)
        .unwrap();
    let next = json!({"cursor": cursor, "limit": LIMIT}).to_string();
    let page = body(&post(&server.addr, ["/messages/list", ""], &next).unwrap()).unwrap();
    assert_eq!(keys(&page), ["dingtalk:acct:m1"]);
    assert_eq!(page.get("next_cursor"), Some(&Value::Null));
    let bad = post(&server.addr, ["/messages/list", ""], r#"{"limit":0}"#).unwrap();
    assert!(bad.starts_with("HTTP/1.1 400"));
    assert!(stop(server).unwrap());
}

#[test]
fn lists_messages_within_window() {
    let path = seeded("cli-messages-window").unwrap();
    let server = start(&path).unwrap();
    let window = r#"{"from_ms":15,"to_ms":30}"#;
    let reply = body(&post(&server.addr, ["/messages/list", ""], window).unwrap()).unwrap();
    assert_eq!(keys(&reply), ["dingtalk:acct:m2"]);
    let inverted = r#"{"from_ms":30,"to_ms":15}"#;
    let bad = post(&server.addr, ["/messages/list", ""], inverted).unwrap();
    assert!(bad.starts_with("HTTP/1.1 400"));
    assert!(stop(server).unwrap());
}

fn seeded(name: &str) -> Fallible<String> {
    let dir = sandbox(name)?;
    let store = Archive::open(Path::new(&format!("{dir}/store.db")))?;
    for (key, millis) in [
        ("dingtalk:acct:m1", 10),
        ("dingtalk:acct:m2", 20),
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
    let response = post(&server.addr, ["/messages/list", ""], "{}").unwrap();
    assert!(response.starts_with("HTTP/1.1 503"));
    assert_eq!(body(&response).unwrap(), json!({"code": "unavailable"}));
    let health = post(&server.addr, ["/health/check", ""], "{}").unwrap();
    assert!(health.starts_with("HTTP/1.1 200"));
    assert!(stop(server).unwrap());
}

#[tokio::test]
async fn storage_failure_uses_configured_exit_code() {
    let path = config("cli-messages-call.yaml", YAML).unwrap();
    let code = call(&path, ["messages", "list"], "{}").await;
    assert_eq!(code, ExitCode::from(15));
}
