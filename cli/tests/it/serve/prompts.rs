use core::error::Error;
use std::fs;

use serde_json::{Value, from_str, json};

use super::{get, send, start, stop};
use crate::support::{config, live, sandbox};

type Fallible<T> = Result<T, Box<dyn Error>>;

fn body(response: &str) -> Fallible<Value> {
    let (_, text) = response
        .split_once("\r\n\r\n")
        .ok_or("response without body")?;
    Ok(from_str(text)?)
}

#[test]
fn prompts_are_listed_and_saved_over_http() {
    let dir = sandbox("cli-prompts-http").unwrap();
    let path = config("cli-prompts-http.yaml", &live(&dir)).unwrap();
    let server = start(&path).unwrap();
    let addr = &server.addr;
    let listed = get(addr, "/api/prompts").unwrap();
    assert!(listed.starts_with("HTTP/1.1 200"));
    let first = body(&listed).unwrap().pointer("/items/0").cloned();
    assert_eq!(
        first,
        Some(json!({"name": "followup", "required": ["{messages}"], "text": "f {messages}"}))
    );
    let saved = send(
        addr,
        ["PATCH", "/api/prompts/urgent", ""],
        r#"{"text":"now {line}"}"#,
    )
    .unwrap();
    assert_eq!(body(&saved).unwrap(), json!({"name": "urgent"}));
    assert_eq!(
        fs::read_to_string(format!("{dir}/prompts/urgent.md")).unwrap(),
        "now {line}"
    );
    let refused = send(
        addr,
        ["PATCH", "/api/prompts/urgent", ""],
        r#"{"text":"now"}"#,
    )
    .unwrap();
    assert!(refused.starts_with("HTTP/1.1 422"));
    let missing = send(
        addr,
        ["PATCH", "/api/prompts/screen", ""],
        r#"{"text":"judge"}"#,
    )
    .unwrap();
    assert!(missing.starts_with("HTTP/1.1 404"));
    fs::remove_dir_all(format!("{dir}/prompts")).unwrap();
    let down = get(addr, "/api/prompts").unwrap();
    assert!(down.starts_with("HTTP/1.1 503"));
    assert!(stop(server).unwrap());
}
