use adapters::store::Shelf;
use domain::batches::Roster as _;
use domain::review::Listing;
use serde_json::{Value, json, to_value};
use tracing::dispatcher::with_default;

use super::rig::{BATCHES, logged, one, seed, start, table};
use crate::capture::Capture;
use crate::support::{BUSY, Sandbox};

const BATCH_ROWS: &str = "INSERT INTO batches (id, state, payload, layout, attempts, reason, created_at_ms, updated_at_ms) VALUES (7, 'sent', '', '', 0, '', 1, 1), (8, 'failed', '', '', 2, 'http:500', 1, 1);";
const LEGACY: &str = "CREATE TABLE batches (id INTEGER PRIMARY KEY NOT NULL, state TEXT NOT NULL, payload TEXT NOT NULL, layout TEXT NOT NULL, attempts INTEGER NOT NULL, reason TEXT NOT NULL, created_at_ms INTEGER NOT NULL, updated_at_ms INTEGER NOT NULL); INSERT INTO batches VALUES (1, 'failed', '', '', 2, 'http:500', 1, 1);";
const LIMIT: u32 = 5;
const LONG: u64 = 4_000;
const ONE: u32 = 1;

#[tokio::test]
async fn failed_batches_are_revived_up_to_the_limit() {
    let sandbox = Sandbox::new("revive-auto").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({
        "limits": {"max_revivals": ONE},
        "stub": {"decide": {"mode": "fail"}},
    });
    let logs = logged(&sandbox, &patch, LONG).await.unwrap();
    assert_eq!(table(&sandbox, BATCHES).unwrap(), ["1|failed|4|http:500"]);
    assert_eq!(logs.matches("failed batch revived").count(), 1);
    assert!(logs.contains(r#""exhausted":true"#));
}

fn ids(sandbox: &Sandbox, state: &str) -> Vec<u64> {
    let path = sandbox.database();
    Shelf::new(&path, BUSY)
        .list(&Listing::new(state, LIMIT))
        .unwrap_or_default()
        .iter()
        .filter_map(|item| to_value(item).ok()?.get("batch_id").and_then(Value::as_u64))
        .collect()
}

#[tokio::test]
async fn legacy_failed_batches_wait_for_a_person() {
    let sandbox = Sandbox::new("revive-legacy").unwrap();
    sandbox.seed(LEGACY).unwrap();
    start(&sandbox, &json!({"limits": {"max_revivals": ONE}}), LONG)
        .await
        .unwrap();
    assert_eq!(table(&sandbox, BATCHES).unwrap(), ["1|failed|2|http:500"]);
    assert_eq!(ids(&sandbox, "failed"), [1]);
    let path = sandbox.database();
    let shelf = Shelf::new(&path, BUSY);
    assert_eq!(shelf.revive(1).unwrap().as_deref(), Some("ready"));
    start(&sandbox, &json!({}), LONG).await.unwrap();
    assert_eq!(table(&sandbox, BATCHES).unwrap(), ["1|sent|0|"]);
}

#[tokio::test]
async fn manual_revival_respects_state() {
    let sandbox = Sandbox::new("revive-manual").unwrap();
    start(&sandbox, &json!({}), LONG).await.unwrap();
    sandbox.seed(BATCH_ROWS).unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, BUSY);
    assert_eq!(shelf.revive(7).unwrap().as_deref(), Some("sent"));
    assert_eq!(shelf.revive(9).unwrap(), None);
    sandbox
        .seed("INSERT INTO batches (id, state, payload, layout, attempts, reason, created_at_ms, updated_at_ms) VALUES (9, 'ready', '', '', 0, '', 1, 1);")
        .unwrap();
    assert_eq!(shelf.revive(8).unwrap().as_deref(), Some("failed"));
    assert_eq!(ids(&sandbox, "sent"), [7]);
}

#[test]
fn roster_reports_storage_failures() {
    let sandbox = Sandbox::new("revive-broken").unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, BUSY);
    let capture = Capture::default();
    with_default(&capture.dispatch(), || {
        assert_eq!(shelf.revive(1).unwrap_err().status(), 503);
        assert_eq!(
            shelf
                .list(&Listing::new("failed", LIMIT))
                .unwrap_err()
                .status(),
            503
        );
    });
    assert!(capture.text().contains("batch revive failed"));
    assert!(capture.text().contains("batches read failed"));
    sandbox.seed("CREATE TABLE other (id INTEGER);").unwrap();
    assert_eq!(
        shelf
            .list(&Listing::new("failed", LIMIT))
            .unwrap_err()
            .status(),
        503
    );
    sandbox
        .seed("CREATE TABLE batches (id INTEGER, state TEXT, attempts TEXT, reason TEXT, updated_at_ms INTEGER, revivals INTEGER); CREATE TABLE lines (batch_id INTEGER, state TEXT); INSERT INTO batches VALUES (1, 'failed', 'x', '', 1, 0); CREATE TRIGGER refuse BEFORE UPDATE ON batches BEGIN SELECT RAISE(ABORT, 'refused'); END;")
        .unwrap();
    assert_eq!(shelf.revive(1).unwrap_err().status(), 503);
    assert_eq!(
        shelf
            .list(&Listing::new("failed", LIMIT))
            .unwrap_err()
            .status(),
        503
    );
}
