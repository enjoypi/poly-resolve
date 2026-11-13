use adapters::store::Archive;
use serde_json::json;

use crate::feeds::{event, feed, page};
use crate::support::{
    BAD_SCHEMA, BUSY, FAILURE, NOON, REFUSE_INSERT, REFUSE_UPDATE, Sandbox, TEXT_STAMP, run_for,
    sealed_envelope,
};

const LONG: u64 = 20_000;

#[tokio::test]
async fn store_failure_is_fatal_for_events() {
    let sandbox = Sandbox::new("ingest-store-events").unwrap();
    drop(Archive::open(&sandbox.database(), BUSY).unwrap());
    sandbox.seed(REFUSE_INSERT).unwrap();
    sandbox.write("listen.txt", &feed(&[event("m1")])).unwrap();
    let config = sandbox.config(&json!({})).unwrap();
    assert_eq!(run_for(&config, LONG).await.unwrap(), FAILURE);
}

#[tokio::test]
async fn store_failure_is_fatal_for_heal() {
    let sandbox = Sandbox::new("ingest-store-heal").unwrap();
    sandbox.seed(BAD_SCHEMA).unwrap();
    let config = sandbox.config(&json!({})).unwrap();
    assert_eq!(run_for(&config, LONG).await.unwrap(), FAILURE);
}

#[tokio::test]
async fn store_failure_is_fatal_for_pages() {
    let sandbox = Sandbox::new("ingest-store-pages").unwrap();
    drop(Archive::open(&sandbox.database(), BUSY).unwrap());
    sandbox.seed(REFUSE_INSERT).unwrap();
    sandbox.write("pull.json", &page(&["m1"], None)).unwrap();
    let config = sandbox.config(&json!({})).unwrap();
    assert_eq!(run_for(&config, LONG).await.unwrap(), FAILURE);
}

#[tokio::test]
async fn store_failure_is_fatal_for_replace() {
    let sandbox = Sandbox::new("ingest-store-replace").unwrap();
    let store = Archive::open(&sandbox.database(), BUSY).unwrap();
    assert_eq!(store.save_event(&sealed_envelope("k1", NOON)).unwrap(), 1);
    sandbox.seed(REFUSE_UPDATE).unwrap();
    let config = sandbox.config(&json!({})).unwrap();
    assert_eq!(run_for(&config, LONG).await.unwrap(), FAILURE);
}

#[tokio::test]
async fn store_failure_is_fatal_for_watermark() {
    let sandbox = Sandbox::new("ingest-store-watermark").unwrap();
    sandbox.seed(TEXT_STAMP).unwrap();
    let config = sandbox.config(&json!({})).unwrap();
    assert_eq!(run_for(&config, LONG).await.unwrap(), FAILURE);
}

#[tokio::test]
async fn store_open_failure_is_fatal() {
    let sandbox = Sandbox::new("ingest-store-open").unwrap();
    let config = sandbox
        .config(&json!({"store_path": "/nonexistent/dir/store.db"}))
        .unwrap();
    assert_eq!(run_for(&config, LONG).await.unwrap(), FAILURE);
}
