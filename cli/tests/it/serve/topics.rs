use core::error::Error;
use std::fs;
use std::path::Path;

use adapters::store::Archive;
use rusqlite::Connection;
use serde_json::{Value, from_str, json};

use super::{get, start, stop};
use crate::support::{BUSY, await_log, config, live, sandbox};

const ROWS: &str = "INSERT INTO topics (id, label, probability, thread_id, until_ms, until_key, thread_at_ms, created_at_ms, last_at_ms) VALUES ('t1', 'first', 0.5, '', 0, '', 0, 1, 5), ('t2', 'second', 0.25, '', 0, '', 0, 1, 9); UPDATE topics SET state = 'resolved' WHERE id = 't2'; INSERT INTO lines (key, conversation_id, occurred_at_ms, sender, text, state, reason, reviewer, flag, hints, rules_rev, batch_id, created_at_ms, updated_at_ms) VALUES ('m1', 'c1', 1, 'alice', 'line one', 'analysed', '', 'auto', '', '[]', 0, 1, 1, 1); INSERT INTO members VALUES ('m1', 't1', 0.5, 1, 'new', 1);";

type Fallible<T> = Result<T, Box<dyn Error>>;

fn body(response: &str) -> Fallible<Value> {
    let (_, text) = response
        .split_once("\r\n\r\n")
        .ok_or("response without body")?;
    Ok(from_str(text)?)
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
    let dir = sandbox("cli-topics-down").unwrap();
    let path = config("cli-topics-down.yaml", &live(&dir)).unwrap();
    let mut server = start(&path).unwrap();
    await_log(&mut server.lines, "page rejected").unwrap();
    fs::remove_file(format!("{dir}/store.db")).unwrap();
    let listed = get(&server.addr, "/api/topics").unwrap();
    assert!(listed.starts_with("HTTP/1.1 503"));
    let found = get(&server.addr, "/api/topics/t1").unwrap();
    assert!(found.starts_with("HTTP/1.1 503"));
    assert!(stop(server).unwrap());
}

#[test]
fn topics_are_read_over_http() {
    let path = seeded("cli-topics-http").unwrap();
    let server = start(&path).unwrap();
    let addr = &server.addr;
    let listed = get(addr, "/api/topics?state=open").unwrap();
    assert!(listed.starts_with("HTTP/1.1 200"));
    let reply = body(&listed).unwrap();
    assert_eq!(reply.pointer("/items/0/topic_id"), Some(&json!("t1")));
    assert_eq!(reply.pointer("/items/1"), None);
    assert_eq!(reply.pointer("/topics/1/state"), Some(&json!("resolved")));
    let found = get(addr, "/api/topics/t1").unwrap();
    assert!(found.starts_with("HTTP/1.1 200"));
    let detail = body(&found).unwrap();
    assert_eq!(detail.pointer("/timeline/0/key"), Some(&json!("m1")));
    let missing = get(addr, "/api/topics/none").unwrap();
    assert!(missing.starts_with("HTTP/1.1 404"));
    let refused = get(addr, "/api/topics?limit=0").unwrap();
    assert!(refused.starts_with("HTTP/1.1 400"));
    assert!(stop(server).unwrap());
}
