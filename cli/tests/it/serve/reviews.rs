use core::error::Error;
use std::fs;
use std::path::Path;

use adapters::store::Archive;
use rusqlite::Connection;
use serde_json::{Value, from_str, json};

use super::{post, start, stop};
use crate::support::{await_log, config, live, sandbox};

const ROWS: &str = "INSERT INTO lines (key, conversation_id, occurred_at_ms, sender, text, state, reason, reviewer, flag, hints, rules_rev, batch_id, created_at_ms, updated_at_ms) VALUES ('a', 'c1', 1, 'alice', 'text a secret-abc', 'pending', '', '', '', '[]', 0, 1, 1, 1), ('b', 'c1', 2, 'alice', 'text b', 'pending', '', '', '', '[]', 0, 1, 2, 2), ('c', 'c1', 3, 'alice', 'text c secret-abc', 'pending', '', '', '', '[]', 0, 1, 3, 3);";
const USER: &str = "x-user: alice\r\n";

type Fallible<T> = Result<T, Box<dyn Error>>;

fn body(response: &str) -> Fallible<Value> {
    let (_, text) = response
        .split_once("\r\n\r\n")
        .ok_or("response without body")?;
    Ok(from_str(text)?)
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
fn reviews_are_listed_and_ruled_over_http() {
    let path = seeded("cli-reviews-http").unwrap();
    let server = start(&path).unwrap();
    let addr = &server.addr;
    let listed = post(addr, ["/review/list", ""], r#"{"limit":3}"#).unwrap();
    assert!(listed.starts_with("HTTP/1.1 200"));
    let reply = body(&listed).unwrap();
    assert_eq!(keys(&reply), ["a", "b", "c"]);
    assert_eq!(
        reply.get("reasons"),
        Some(&json!([
            {"code": "leak", "label": "Leaked", "placeholder": "[x]"},
            {"code": "noise", "label": "Noise", "placeholder": null},
        ]))
    );
    let anonymous = post(addr, ["/review/approve", ""], r#"{"keys":["a"]}"#).unwrap();
    assert!(anonymous.starts_with("HTTP/1.1 400"));
    let approved = post(addr, ["/review/approve", USER], r#"{"keys":["a"]}"#).unwrap();
    assert_eq!(body(&approved).unwrap(), json!({"states": ["approved"]}));
    let unknown = r#"{"key":"b","reason":"nope"}"#;
    let refused = post(addr, ["/review/reject", USER], unknown).unwrap();
    assert!(refused.starts_with("HTTP/1.1 400"));
    let known = r#"{"key":"b","reason":"noise"}"#;
    let rejected = post(addr, ["/review/reject", USER], known).unwrap();
    assert_eq!(body(&rejected).unwrap(), json!({"state": "rejected"}));
    let bare = r#"{"key":"c","reason":"leak"}"#;
    let missing_leak = post(addr, ["/review/reject", USER], bare).unwrap();
    assert!(missing_leak.starts_with("HTTP/1.1 400"));
    let leak = r#"{"key":"c","leak":"secret-abc","reason":"leak"}"#;
    let leaked = post(addr, ["/review/reject", USER], leak).unwrap();
    assert!(!leaked.contains("secret-abc"));
    assert_eq!(body(&leaked).unwrap(), json!({"state": "leaked"}));
    let missing = post(addr, ["/review/approve", USER], r#"{"keys":["nope"]}"#).unwrap();
    assert!(missing.starts_with("HTTP/1.1 400"));
    let done = post(addr, ["/review/list", ""], r#"{"state":"approved"}"#).unwrap();
    assert_eq!(keys(&body(&done).unwrap()), ["a"]);
    assert!(stop(server).unwrap());
}

fn seeded(name: &str) -> Fallible<String> {
    let dir = sandbox(name)?;
    let database = format!("{dir}/store.db");
    drop(Archive::open(Path::new(&database))?);
    Connection::open(&database)?.execute_batch(ROWS)?;
    Ok(config(&format!("{name}.yaml"), &live(&dir))?)
}

#[test]
fn storage_failure_is_service_unavailable() {
    let dir = sandbox("cli-reviews-down").unwrap();
    let path = config("cli-reviews-down.yaml", &live(&dir)).unwrap();
    let mut server = start(&path).unwrap();
    await_log(&mut server.lines, "page rejected").unwrap();
    fs::remove_file(format!("{dir}/store.db")).unwrap();
    let listed = post(&server.addr, ["/review/list", ""], "{}").unwrap();
    assert!(listed.starts_with("HTTP/1.1 503"));
    let ruled = post(&server.addr, ["/review/approve", USER], r#"{"keys":["a"]}"#).unwrap();
    assert!(ruled.starts_with("HTTP/1.1 503"));
    let leak = r#"{"key":"a","leak":"secret-abc","reason":"leak"}"#;
    let sourced = post(&server.addr, ["/review/reject", USER], leak).unwrap();
    assert!(sourced.starts_with("HTTP/1.1 503"));
    assert!(stop(server).unwrap());
}
