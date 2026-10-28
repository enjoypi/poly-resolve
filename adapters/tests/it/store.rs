use adapters::store::{Archive, Scope, Shelf};
use domain::inbox::{Mark, Query, Reader as _, Row, Window};
use serde_json::{Value, from_str, json};

use crate::support::{
    BAD_SCHEMA, BAD_STAMP, NOON, Sandbox, TEXT_STAMP, envelope, keys, sealed_envelope, texts,
};

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
fn replace_swaps_text_only_while_unchanged() {
    let sandbox = Sandbox::new("store-replace").unwrap();
    let store = Archive::open(&sandbox.database()).unwrap();
    let stale = sealed_envelope("k1", NOON);
    assert_eq!(store.save_event(&stale).unwrap(), 1);
    let fresh = envelope("k1", NOON);
    assert_eq!(store.replace(&stale, &fresh).unwrap(), 1);
    assert_eq!(store.replace(&stale, &fresh).unwrap(), 0);
    assert_eq!(texts(&sandbox.database()).unwrap(), ["hello"]);
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
fn sealed_lists_candidates_newest_first_within_limit() {
    let sandbox = Sandbox::new("store-sealed").unwrap();
    let store = Archive::open(&sandbox.database()).unwrap();
    for (key, millis) in [("k1", NOON), ("k2", NOON + 5)] {
        assert_eq!(store.save_event(&sealed_envelope(key, millis)).unwrap(), 1);
    }
    assert_eq!(store.save_event(&envelope("k3", NOON + 10)).unwrap(), 1);
    sandbox.seed(OTHER).unwrap();
    let listed = |limit| {
        store
            .sealed(Scope::new("dingtalk", "acct"), limit)
            .unwrap()
            .iter()
            .map(|row| from_str::<Value>(row).unwrap().get("key").cloned())
            .collect::<Vec<_>>()
    };
    assert_eq!(listed(PAGE), [Some(json!("k2")), Some(json!("k1"))]);
    assert_eq!(listed(1), [Some(json!("k2"))]);
    let other = store.sealed(Scope::new("dingtalk", "other"), PAGE).unwrap();
    assert_eq!(other, Vec::<String>::new());
}

#[test]
fn shelf_fails_on_incompatible_schema() {
    let sandbox = Sandbox::new("shelf-incompatible").unwrap();
    sandbox.seed(BAD_SCHEMA).unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, "dingtalk", "acct");
    let err = shelf
        .page(&Query::new(None, PAGE, Window::default()))
        .unwrap_err();
    assert_eq!(err.status(), 503);
}

#[test]
fn shelf_fails_on_missing_database() {
    let sandbox = Sandbox::new("shelf-missing").unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, "dingtalk", "acct");
    let err = shelf
        .page(&Query::new(None, PAGE, Window::default()))
        .unwrap_err();
    assert_eq!(err.status(), 503);
    assert!(!sandbox.database().exists());
}

#[test]
fn shelf_fails_on_undecodable_row() {
    let sandbox = Sandbox::new("shelf-undecodable").unwrap();
    sandbox.seed(BAD_STAMP).unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, "dingtalk", "acct");
    let err = shelf
        .page(&Query::new(None, PAGE, Window::default()))
        .unwrap_err();
    assert_eq!(err.status(), 503);
}

#[test]
fn shelf_filters_by_half_open_window() {
    let sandbox = Sandbox::new("shelf-window").unwrap();
    let store = Archive::open(&sandbox.database()).unwrap();
    for (key, millis) in [("k1", NOON), ("k2", NOON + 5), ("k3", NOON + 10)] {
        assert_eq!(store.save_event(&envelope(key, millis)).unwrap(), 1);
    }
    let path = sandbox.database();
    let shelf = Shelf::new(&path, "dingtalk", "acct");
    let page = |since, until| {
        shelf
            .page(&Query::new(None, PAGE, Window::new(since, until)))
            .unwrap()
    };
    assert_eq!(ids(&page(Some(NOON + 5), None)), ["k3", "k2"]);
    assert_eq!(ids(&page(None, Some(NOON + 10))), ["k2", "k1"]);
    assert_eq!(ids(&page(Some(NOON + 5), Some(NOON + 10))), ["k2"]);
    assert!(page(Some(u64::MAX), None).is_empty());
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
    let all = shelf
        .page(&Query::new(None, PAGE, Window::default()))
        .unwrap();
    assert_eq!(ids(&all), ["k3", "k2", "k1"]);
    let first = shelf.page(&Query::new(None, 1, Window::default())).unwrap();
    assert_eq!(ids(&first), ["k3"]);
    let after = Mark::new("k2".to_owned(), NOON + 5);
    let rest = shelf
        .page(&Query::new(Some(after), PAGE, Window::default()))
        .unwrap();
    assert_eq!(ids(&rest), ["k1"]);
    let beyond = Mark::new("k1".to_owned(), NOON);
    assert!(
        shelf
            .page(&Query::new(Some(beyond), PAGE, Window::default()))
            .unwrap()
            .is_empty()
    );
    let other = Shelf::new(&path, "dingtalk", "other");
    assert_eq!(
        ids(&other
            .page(&Query::new(None, PAGE, Window::default()))
            .unwrap()),
        ["x1"]
    );
    let slack = Shelf::new(&path, "slack", "acct");
    assert_eq!(
        ids(&slack
            .page(&Query::new(None, PAGE, Window::default()))
            .unwrap()),
        ["x2"]
    );
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
