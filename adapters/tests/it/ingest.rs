use std::io::ErrorKind;

use adapters::store::Archive;
use serde_json::json;

use crate::support::{
    BAD_SCHEMA, FAILURE, MANUAL, NOON, Sandbox, TEXT_STAMP, envelope, event, feed, keys, page,
    run_for, run_logged,
};

const HUGE: u64 = 9_000_000_000_000_000_000;
const LONG: u64 = 20_000;
const LONG_BACKOFF: u64 = 60_000;
const SHORT: u64 = 700;
const SHORT_CHILD: u64 = 100;

#[tokio::test]
async fn events_and_pages_are_merged_without_duplicates() {
    let sandbox = Sandbox::new("ingest-merge").unwrap();
    let lines = [event("m1"), event("m2"), "not json".to_owned(), event("m1")];
    sandbox.write("listen.txt", &feed(&lines)).unwrap();
    sandbox
        .write("pull.json", &page(&["m2", "m3"], Some("next")))
        .unwrap();
    let config = sandbox.config(&json!({})).unwrap();
    let (code, logs) = run_logged(&config, SHORT).await.unwrap();
    assert_eq!(code, 0);
    assert!(logs.contains("reconciled"));
    assert!(!logs.contains("hello"));
    assert_eq!(
        keys(&sandbox.database()).unwrap(),
        ["dingtalk:acct:m1", "dingtalk:acct:m2", "dingtalk:acct:m3"]
    );
    let args = sandbox.read("pull-args.txt").unwrap();
    let words: Vec<&str> = args.split_whitespace().collect();
    assert_eq!(words.len(), 7);
    assert_eq!(words.first(), Some(&"pull"));
    assert_eq!(words.get(1), Some(&"--start"));
    assert_eq!(words.get(4), Some(&"--end"));
}

#[tokio::test]
async fn kills_unresponsive_listener() {
    let sandbox = Sandbox::new("ingest-kill").unwrap();
    let config = sandbox
        .config(&json!({
            "child_exit_timeout_ms": SHORT_CHILD,
            "dws": {"env": {"FAKE_LISTEN": "hang"}},
        }))
        .unwrap();
    assert_eq!(run_for(&config, SHORT).await.unwrap(), 0);
}

#[tokio::test]
async fn listen_ending_with_events_restarts() {
    let sandbox = Sandbox::new("ingest-restart").unwrap();
    sandbox.write("listen.txt", &feed(&[event("m1")])).unwrap();
    let config = sandbox
        .config(&json!({"dws": {"env": {"FAKE_LISTEN": "exit"}}}))
        .unwrap();
    assert_eq!(run_for(&config, SHORT).await.unwrap(), 0);
    assert_eq!(keys(&sandbox.database()).unwrap(), ["dingtalk:acct:m1"]);
}

#[tokio::test]
async fn listen_ending_without_events_backs_off() {
    let sandbox = Sandbox::new("ingest-backoff").unwrap();
    let config = sandbox
        .config(&json!({"dws": {"env": {"FAKE_LISTEN": "exit"}}}))
        .unwrap();
    assert_eq!(run_for(&config, SHORT).await.unwrap(), 0);
    assert_eq!(keys(&sandbox.database()).unwrap(), Vec::<String>::new());
}

#[tokio::test]
async fn listen_exit_code_requires_manual_action() {
    let sandbox = Sandbox::new("ingest-listen-manual").unwrap();
    let config = sandbox
        .config(&json!({"dws": {"env": {"FAKE_LISTEN": "exit", "FAKE_CODE": "2"}}}))
        .unwrap();
    assert_eq!(run_for(&config, LONG).await.unwrap(), MANUAL);
}

#[tokio::test]
async fn missing_listen_binary_backs_off() {
    let sandbox = Sandbox::new("ingest-no-listen").unwrap();
    let config = sandbox
        .config(&json!({"dws": {"listen": {"bin": "/nonexistent/dws"}}}))
        .unwrap();
    assert_eq!(run_for(&config, SHORT).await.unwrap(), 0);
}

#[tokio::test]
async fn missing_pull_binary_is_not_fatal() {
    let sandbox = Sandbox::new("ingest-no-pull").unwrap();
    sandbox.write("listen.txt", &feed(&[event("m1")])).unwrap();
    let config = sandbox
        .config(&json!({"dws": {"pull": {"bin": "/nonexistent/dws"}}}))
        .unwrap();
    assert_eq!(run_for(&config, SHORT).await.unwrap(), 0);
    assert_eq!(keys(&sandbox.database()).unwrap(), ["dingtalk:acct:m1"]);
}

#[tokio::test]
async fn pull_failure_is_not_fatal() {
    let sandbox = Sandbox::new("ingest-pull-fail").unwrap();
    sandbox.write("listen.txt", &feed(&[event("m1")])).unwrap();
    let config = sandbox
        .config(&json!({"dws": {"env": {"FAKE_PULL": "fail"}}}))
        .unwrap();
    assert_eq!(run_for(&config, SHORT).await.unwrap(), 0);
    assert_eq!(keys(&sandbox.database()).unwrap(), ["dingtalk:acct:m1"]);
}

#[tokio::test]
async fn pull_requiring_manual_action_stops() {
    let sandbox = Sandbox::new("ingest-pull-manual").unwrap();
    let config = sandbox
        .config(&json!({"dws": {"env": {"FAKE_PULL": "fail", "FAKE_CODE": "4"}}}))
        .unwrap();
    assert_eq!(run_for(&config, LONG).await.unwrap(), MANUAL);
}

#[tokio::test]
async fn pull_with_corrupt_page_is_not_fatal() {
    let sandbox = Sandbox::new("ingest-pull-corrupt").unwrap();
    sandbox.write("listen.txt", &feed(&[event("m1")])).unwrap();
    sandbox.write("pull.json", "not json").unwrap();
    let config = sandbox.config(&json!({})).unwrap();
    assert_eq!(run_for(&config, SHORT).await.unwrap(), 0);
    assert_eq!(keys(&sandbox.database()).unwrap(), ["dingtalk:acct:m1"]);
}

#[tokio::test]
async fn seeded_watermark_bounds_pull_window() {
    let sandbox = Sandbox::new("ingest-watermark").unwrap();
    let store = Archive::open(&sandbox.database()).unwrap();
    assert_eq!(store.save_event(&envelope("seed", NOON)).unwrap(), 1);
    let config = sandbox.config(&json!({})).unwrap();
    assert_eq!(run_for(&config, SHORT).await.unwrap(), 0);
    let args = sandbox.read("pull-args.txt").unwrap();
    assert!(args.contains("--start 2026-07-08 12:00:34 "));
}

#[tokio::test]
async fn stop_during_backoff_ends_run() {
    let sandbox = Sandbox::new("ingest-stop-backoff").unwrap();
    let config = sandbox
        .config(&json!({
            "backoff_initial_ms": LONG_BACKOFF,
            "backoff_max_ms": LONG_BACKOFF,
            "dws": {"env": {"FAKE_LISTEN": "exit"}},
        }))
        .unwrap();
    assert_eq!(run_for(&config, SHORT).await.unwrap(), 0);
}

#[tokio::test]
async fn stop_during_pull_ends_run() {
    let sandbox = Sandbox::new("ingest-stop-pull").unwrap();
    let config = sandbox
        .config(&json!({"dws": {"env": {"FAKE_PULL": "slow"}}}))
        .unwrap();
    assert_eq!(run_for(&config, SHORT).await.unwrap(), 0);
}

#[tokio::test]
async fn store_failure_is_fatal_for_events() {
    let sandbox = Sandbox::new("ingest-store-events").unwrap();
    sandbox.seed(BAD_SCHEMA).unwrap();
    sandbox.write("listen.txt", &feed(&[event("m1")])).unwrap();
    let config = sandbox.config(&json!({})).unwrap();
    assert_eq!(run_for(&config, LONG).await.unwrap(), FAILURE);
}

#[tokio::test]
async fn store_failure_is_fatal_for_pages() {
    let sandbox = Sandbox::new("ingest-store-pages").unwrap();
    sandbox.seed(BAD_SCHEMA).unwrap();
    sandbox.write("pull.json", &page(&["m1"], None)).unwrap();
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

#[tokio::test]
async fn unrepresentable_watermark_skips_reconcile() {
    let sandbox = Sandbox::new("ingest-huge").unwrap();
    let store = Archive::open(&sandbox.database()).unwrap();
    assert_eq!(store.save_event(&envelope("huge", HUGE)).unwrap(), 1);
    sandbox.write("listen.txt", &feed(&[event("m1")])).unwrap();
    let config = sandbox.config(&json!({})).unwrap();
    assert_eq!(run_for(&config, SHORT).await.unwrap(), 0);
    assert_eq!(
        keys(&sandbox.database()).unwrap(),
        ["dingtalk:acct:m1", "huge"]
    );
    assert_eq!(
        sandbox.read("pull-args.txt").unwrap_err().kind(),
        ErrorKind::NotFound
    );
}
