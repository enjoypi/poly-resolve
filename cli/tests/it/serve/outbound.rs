use core::error::Error;
use std::fs;
use std::path::Path;

use adapters::store::Archive;
use rusqlite::Connection;
use serde_json::{Value, from_str, json};

use super::{get, start, stop};
use crate::support::{BUSY, await_log, config, live, sandbox};

const ROWS: &str = "INSERT INTO lines (key, conversation_id, occurred_at_ms, sender, text, state, reason, reviewer, flag, hints, rules_rev, batch_id, created_at_ms, updated_at_ms) VALUES ('a+/b', 'c1', 1, 'alice', 'text a [num]', 'approved', '', 'auto', '', '[]', 0, 1, 1, 1), ('b', 'c1', 2, 'alice', 'text b', 'approved', '', 'auto', '', '[]', 0, 1, 2, 2), ('c', 'c1', 3, '', '', 'withheld', 'sealed', '', '', '[]', 0, 0, 3, 3);";

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
fn outbound_lines_are_listed_over_http() {
    let path = seeded("cli-outbound-http").unwrap();
    let server = start(&path).unwrap();
    let addr = &server.addr;
    let listed = get(addr, "/api/outbound?limit=3").unwrap();
    assert!(listed.starts_with("HTTP/1.1 200"));
    let reply = body(&listed).unwrap();
    assert_eq!(keys(&reply), ["b", "a+/b"]);
    assert_eq!(reply.get("placeholders"), Some(&json!(["[num]"])));
    let held = get(addr, "/api/outbound?state=withheld").unwrap();
    assert_eq!(keys(&body(&held).unwrap()), ["c"]);
    let refused = get(addr, "/api/outbound?limit=0").unwrap();
    assert!(refused.starts_with("HTTP/1.1 400"));
    assert!(stop(server).unwrap());
}

fn seeded(name: &str) -> Fallible<String> {
    let dir = sandbox(name)?;
    let database = format!("{dir}/store.db");
    drop(Archive::open(Path::new(&database), BUSY)?);
    Connection::open(&database)?.execute_batch(ROWS)?;
    Ok(config(&format!("{name}.yaml"), &live(&dir))?)
}

#[test]
fn storage_failure_is_service_unavailable() {
    let dir = sandbox("cli-outbound-down").unwrap();
    let path = config("cli-outbound-down.yaml", &live(&dir)).unwrap();
    let mut server = start(&path).unwrap();
    await_log(&mut server.lines, "page rejected").unwrap();
    fs::remove_file(format!("{dir}/store.db")).unwrap();
    let listed = get(&server.addr, "/api/outbound").unwrap();
    assert!(listed.starts_with("HTTP/1.1 503"));
    assert!(stop(server).unwrap());
}
