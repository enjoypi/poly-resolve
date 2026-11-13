use adapters::store::{Archive, Shelf};
use domain::gauge::Meter as _;
use serde_json::{Value, to_value};

use crate::support::{BUSY, Sandbox};

const DAYS: u32 = 3;
const ROWS: &str = "INSERT INTO source_health VALUES ('imap', 'box', 'retrying', 2, 5, 9); INSERT INTO messages (key, source, account, conversation_id, sender_id, sender_name, text, occurred_at_ms, received_at_ms, raw) VALUES ('m', 'imap', 'box', 'c', 's', 'n', 't', 7, 7, ''); INSERT INTO calls (kind, subject, topic_id, attempt, at_ms, state, input_tokens, output_tokens, version) VALUES ('decision', '1', '', 1, strftime('%s', 'now') * 1000, 'success', 10, 2, 'v'), ('decision', '1', '', 2, strftime('%s', 'now') * 1000 + 1, 'failed', 0, 0, 'v'), ('analysis', 'a', 't', 1, strftime('%s', 'now') * 1000 - 1, 'failed', 0, 0, 'v'), ('notify', 'n', 't', 1, 1, 'success', 0, 0, '');";

#[test]
fn health_and_usage_fail_on_missing_database() {
    let sandbox = Sandbox::new("gauge-missing").unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, BUSY);
    assert_eq!(shelf.sources().unwrap_err().status(), 503);
    assert_eq!(shelf.usage(DAYS).unwrap_err().status(), 503);
}

#[test]
fn health_and_usage_fail_on_undecodable_rows() {
    let sandbox = Sandbox::new("gauge-undecodable").unwrap();
    sandbox
        .seed("CREATE TABLE source_health (source TEXT, account TEXT, state TEXT, streak TEXT, ok_at_ms INTEGER, down_at_ms INTEGER); INSERT INTO source_health VALUES ('imap', 'b', 'ok', 'x', 0, 0); CREATE TABLE messages (source TEXT, account TEXT, occurred_at_ms INTEGER); CREATE TABLE calls (kind TEXT, state TEXT, at_ms INTEGER, input_tokens TEXT, output_tokens INTEGER); INSERT INTO calls VALUES ('decision', 'success', strftime('%s', 'now') * 1000, 'x', 0);")
        .unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, BUSY);
    assert_eq!(shelf.sources().unwrap_err().status(), 503);
    assert_eq!(shelf.usage(DAYS).unwrap_err().status(), 503);
}

#[test]
fn health_and_usage_fail_without_tables() {
    let sandbox = Sandbox::new("gauge-table").unwrap();
    sandbox.seed("CREATE TABLE other (key TEXT);").unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, BUSY);
    assert_eq!(shelf.sources().unwrap_err().status(), 503);
    assert_eq!(shelf.usage(DAYS).unwrap_err().status(), 503);
}

#[test]
fn health_reports_sources_and_stage_streaks() {
    let sandbox = Sandbox::new("gauge-health").unwrap();
    drop(Archive::open(&sandbox.database(), BUSY).unwrap());
    sandbox.seed(ROWS).unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, BUSY);
    let found = to_value(shelf.sources().unwrap()).unwrap();
    assert_eq!(
        peek(&found, "/sources"),
        r#"[{"account":"box","down_at_ms":9,"kind":"imap","last_at_ms":7,"ok_at_ms":5,"state":"retrying","streak":2}]"#
    );
    assert_eq!(
        peek(&found, "/stages"),
        r#"[{"kind":"analysis","streak":1},{"kind":"decision","streak":1},{"kind":"notify","streak":0},{"kind":"screen","streak":0}]"#
    );
    let days = to_value(shelf.usage(DAYS).unwrap()).unwrap();
    assert_eq!(peek(&days, "/0/kind"), r#""analysis""#);
    assert_eq!(peek(&days, "/1/calls"), "2");
    assert_eq!(peek(&days, "/1/failures"), "1");
    assert_eq!(peek(&days, "/1/input_tokens"), "10");
    assert_eq!(peek(&days, "/2"), "");
}

fn peek(value: &Value, pointer: &str) -> String {
    value.pointer(pointer).map_or_default(Value::to_string)
}
