use adapters::store::Archive;

use crate::support::{BAD_SCHEMA, NOON, Sandbox, TEXT_STAMP, envelope, keys};

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
