use core::error::Error;
use std::fs;
use std::path::Path;

use adapters::store::Archive;
use rusqlite::Connection;
use serde_json::{Value, from_str, json};

use super::{get, send, start, stop};
use crate::support::{await_log, config, live, sandbox};

const ROWS: &str = "INSERT INTO lines (key, conversation_id, occurred_at_ms, sender, text, state, reason, reviewer, flag, hints, rules_rev, batch_id, created_at_ms, updated_at_ms) VALUES ('a+/b', 'c1', 1, 'alice', 'text a secret-abc', 'pending', '', '', '', '[]', 0, 1, 1, 1), ('b', 'c1', 2, 'alice', 'text b', 'pending', '', '', '', '[]', 0, 1, 2, 2), ('c', 'c1', 3, 'alice', 'text c secret-abc', 'pending', '', '', '', '[]', 0, 1, 3, 3);";

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
    let listed = get(addr, "/api/reviews?limit=3").unwrap();
    assert!(listed.starts_with("HTTP/1.1 200"));
    let reply = body(&listed).unwrap();
    assert_eq!(keys(&reply), ["a+/b", "b", "c"]);
    assert_eq!(
        reply.get("reasons"),
        Some(&json!([
            {"code": "leak", "label": "Leaked", "placeholder": "[x]"},
            {"code": "noise", "label": "Noise", "placeholder": null},
        ]))
    );
    let approve = r#"{"state":"approved"}"#;
    let approved = send(addr, ["PATCH", "/api/reviews/a%2B%2Fb", ""], approve).unwrap();
    assert_eq!(body(&approved).unwrap(), json!({"state": "approved"}));
    let again = send(addr, ["PATCH", "/api/reviews/a%2B%2Fb", ""], approve).unwrap();
    assert_eq!(body(&again).unwrap(), json!({"state": "approved"}));
    let unknown = r#"{"state":"rejected","reason":"nope"}"#;
    let refused = send(addr, ["PATCH", "/api/reviews/b", ""], unknown).unwrap();
    assert!(refused.starts_with("HTTP/1.1 422"));
    let known = r#"{"state":"rejected","reason":"noise"}"#;
    let rejected = send(addr, ["PATCH", "/api/reviews/b", ""], known).unwrap();
    assert_eq!(body(&rejected).unwrap(), json!({"state": "rejected"}));
    let clash = send(addr, ["PATCH", "/api/reviews/b", ""], approve).unwrap();
    assert!(clash.starts_with("HTTP/1.1 409"));
    let bare = r#"{"state":"rejected","reason":"leak"}"#;
    let missing_leak = send(addr, ["PATCH", "/api/reviews/c", ""], bare).unwrap();
    assert!(missing_leak.starts_with("HTTP/1.1 422"));
    let leak = r#"{"state":"rejected","leak":"secret-abc","reason":"leak"}"#;
    let leaked = send(addr, ["PATCH", "/api/reviews/c", ""], leak).unwrap();
    assert!(!leaked.contains("secret-abc"));
    assert_eq!(body(&leaked).unwrap(), json!({"state": "leaked"}));
    let missing = send(addr, ["PATCH", "/api/reviews/nope", ""], approve).unwrap();
    assert!(missing.starts_with("HTTP/1.1 404"));
    let done = get(addr, "/api/reviews?state=approved").unwrap();
    assert_eq!(keys(&body(&done).unwrap()), ["a+/b"]);
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
    let listed = get(&server.addr, "/api/reviews").unwrap();
    assert!(listed.starts_with("HTTP/1.1 503"));
    let approve = r#"{"state":"approved"}"#;
    let ruled = send(&server.addr, ["PATCH", "/api/reviews/a", ""], approve).unwrap();
    assert!(ruled.starts_with("HTTP/1.1 503"));
    let leak = r#"{"state":"rejected","leak":"secret-abc","reason":"leak"}"#;
    let sourced = send(&server.addr, ["PATCH", "/api/reviews/a", ""], leak).unwrap();
    assert!(sourced.starts_with("HTTP/1.1 503"));
    assert!(stop(server).unwrap());
}
