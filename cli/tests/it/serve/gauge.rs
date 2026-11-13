use core::error::Error;
use std::fs;

use serde_json::{Value, from_str, json};

use super::{get, start, stop};
use crate::support::{await_log, config, live, sandbox};

type Fallible<T> = Result<T, Box<dyn Error>>;

fn body(response: &str) -> Fallible<Value> {
    let (_, text) = response
        .split_once("\r\n\r\n")
        .ok_or("response without body")?;
    Ok(from_str(text)?)
}

#[test]
fn health_and_usage_are_read_over_http() {
    let dir = sandbox("cli-gauge").unwrap();
    let path = config("cli-gauge.yaml", &live(&dir)).unwrap();
    let mut server = start(&path).unwrap();
    await_log(&mut server.lines, "page rejected").unwrap();
    let health = get(&server.addr, "/api/sources").unwrap();
    assert!(health.starts_with("HTTP/1.1 200"));
    let found = body(&health).unwrap();
    assert_eq!(found.get("sources"), Some(&json!([])));
    assert_eq!(found.pointer("/stages/0/kind"), Some(&json!("analysis")));
    let usage = get(&server.addr, "/api/usage?limit=1").unwrap();
    assert!(usage.starts_with("HTTP/1.1 200"));
    assert_eq!(body(&usage).unwrap(), json!({"days": []}));
    let refused = get(&server.addr, "/api/usage?limit=0").unwrap();
    assert!(refused.starts_with("HTTP/1.1 400"));
    fs::remove_file(format!("{dir}/store.db")).unwrap();
    assert!(
        get(&server.addr, "/api/sources")
            .unwrap()
            .starts_with("HTTP/1.1 503")
    );
    assert!(
        get(&server.addr, "/api/usage")
            .unwrap()
            .starts_with("HTTP/1.1 503")
    );
    assert!(stop(server).unwrap());
}
