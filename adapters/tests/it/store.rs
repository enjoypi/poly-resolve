use adapters::store::{Archive, Shelf};
use domain::inbox::{Mark, Query, Reader as _, Row};

use crate::support::{BAD_SCHEMA, BAD_STAMP, NOON, Sandbox, TEXT_STAMP, envelope, keys};

const OTHER: &str = "INSERT INTO messages (key, source, account, conversation_id, sender_id, sender_name, text, occurred_at_ms, received_at_ms, raw) VALUES ('x1', 'dingtalk', 'other', 'c1', 'u1', 'bob', 'hi', 9, 9, '{}'), ('x2', 'slack', 'acct', 'c1', 'u1', 'bob', 'hi', 9, 9, '{}');";
const PAGE: u32 = 10;

fn ids(rows: &[Row]) -> Vec<&str> {
    rows.iter().map(Row::key).collect()
}

#[test]
fn open_fails_in_missing_directory() {
    let sandbox = Sandbox::new("store-missing-dir").unwrap();
    let err = Archive::open(&sandbox.database().join("nested").join("store.db")).unwrap_err();
    assert!(err.to_string().contains("unable to open"));
}

#[test]
fn open_fails_on_incompatible_schema() {
    let sandbox = Sandbox::new("store-incompatible").unwrap();
    sandbox.seed("CREATE TABLE messages (other TEXT);").unwrap();
    let err = Archive::open(&sandbox.database()).unwrap_err();
    assert!(err.to_string().contains("no such column"));
}

#[test]
fn open_fails_on_non_database_file() {
    let sandbox = Sandbox::new("store-garbage").unwrap();
    sandbox.write("store.db", &"x".repeat(500)).unwrap();
    let err = Archive::open(&sandbox.database()).unwrap_err();
    assert!(err.to_string().contains("not a database"));
}

#[test]
fn save_event_inserts_once() {
    let sandbox = Sandbox::new("store-event").unwrap();
    let store = Archive::open(&sandbox.database()).unwrap();
    let body = envelope("k1", NOON);
    assert_eq!(store.save_event(&body).unwrap(), 1);
    assert_eq!(store.save_event(&body).unwrap(), 0);
    assert_eq!(keys(&sandbox.database()).unwrap(), ["k1"]);
}

#[test]
fn save_fails_without_required_columns() {
    let sandbox = Sandbox::new("store-columns").unwrap();
    sandbox.seed(BAD_SCHEMA).unwrap();
    let store = Archive::open(&sandbox.database()).unwrap();
    let event = store.save_event(&envelope("k1", NOON)).unwrap_err();
    let page = store.save_page(r#"{"envelopes":[]}"#).unwrap_err();
    assert!(event.to_string().contains("no column named"));
    assert!(page.to_string().contains("no column named"));
}

#[test]
fn save_page_inserts_new_only() {
    let sandbox = Sandbox::new("store-page").unwrap();
    let store = Archive::open(&sandbox.database()).unwrap();
    assert_eq!(store.save_event(&envelope("k1", NOON)).unwrap(), 1);
    let page = format!(
        r#"{{"envelopes":[{},{}],"next_cursor":null,"skipped":0}}"#,
        envelope("k1", NOON),
        envelope("k2", NOON)
    );
    assert_eq!(store.save_page(&page).unwrap(), 1);
    assert_eq!(store.save_page(r#"{"other":1}"#).unwrap(), 0);
    assert_eq!(keys(&sandbox.database()).unwrap(), ["k1", "k2"]);
}

#[test]
fn shelf_fails_on_incompatible_schema() {
    let sandbox = Sandbox::new("shelf-incompatible").unwrap();
    sandbox.seed(BAD_SCHEMA).unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, "dingtalk", "acct");
    let err = shelf.page(&Query::new(None, PAGE)).unwrap_err();
    assert_eq!(err.status(), 503);
}

#[test]
fn shelf_fails_on_missing_database() {
    let sandbox = Sandbox::new("shelf-missing").unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, "dingtalk", "acct");
    let err = shelf.page(&Query::new(None, PAGE)).unwrap_err();
    assert_eq!(err.status(), 503);
    assert!(!sandbox.database().exists());
}

#[test]
fn shelf_fails_on_undecodable_row() {
    let sandbox = Sandbox::new("shelf-undecodable").unwrap();
    sandbox.seed(BAD_STAMP).unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, "dingtalk", "acct");
    let err = shelf.page(&Query::new(None, PAGE)).unwrap_err();
    assert_eq!(err.status(), 503);
}

#[test]
fn shelf_pages_newest_first_for_one_account() {
    let sandbox = Sandbox::new("shelf-pages").unwrap();
    let store = Archive::open(&sandbox.database()).unwrap();
    for (key, millis) in [("k1", NOON), ("k2", NOON + 5), ("k3", NOON + 5)] {
        assert_eq!(store.save_event(&envelope(key, millis)).unwrap(), 1);
    }
    sandbox.seed(OTHER).unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, "dingtalk", "acct");
    let all = shelf.page(&Query::new(None, PAGE)).unwrap();
    assert_eq!(ids(&all), ["k3", "k2", "k1"]);
    let first = shelf.page(&Query::new(None, 1)).unwrap();
    assert_eq!(ids(&first), ["k3"]);
    let after = Mark::new("k2".to_owned(), NOON + 5);
    let rest = shelf.page(&Query::new(Some(after), PAGE)).unwrap();
    assert_eq!(ids(&rest), ["k1"]);
    let beyond = Mark::new("k1".to_owned(), NOON);
    assert!(
        shelf
            .page(&Query::new(Some(beyond), PAGE))
            .unwrap()
            .is_empty()
    );
    let other = Shelf::new(&path, "dingtalk", "other");
    assert_eq!(ids(&other.page(&Query::new(None, PAGE)).unwrap()), ["x1"]);
    let slack = Shelf::new(&path, "slack", "acct");
    assert_eq!(ids(&slack.page(&Query::new(None, PAGE)).unwrap()), ["x2"]);
}

#[test]
fn watermark_fails_on_non_numeric_value() {
    let sandbox = Sandbox::new("store-text-stamp").unwrap();
    sandbox.seed(TEXT_STAMP).unwrap();
    let store = Archive::open(&sandbox.database()).unwrap();
    let err = store.watermark("dingtalk", "acct").unwrap_err();
    assert!(err.to_string().contains("Invalid column type"));
}

#[test]
fn watermark_tracks_latest_per_account() {
    let sandbox = Sandbox::new("store-watermark").unwrap();
    let store = Archive::open(&sandbox.database()).unwrap();
    assert_eq!(store.watermark("dingtalk", "acct").unwrap(), None);
    assert_eq!(store.save_event(&envelope("k1", NOON)).unwrap(), 1);
    assert_eq!(store.save_event(&envelope("k2", NOON + 5)).unwrap(), 1);
    assert_eq!(store.watermark("dingtalk", "acct").unwrap(), Some(NOON + 5));
    assert_eq!(store.watermark("dingtalk", "other").unwrap(), None);
}
