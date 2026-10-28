use std::io::ErrorKind;

use adapters::store::Archive;
use serde_json::json;

use crate::support::{
    BAD_SCHEMA, FAILURE, FRAGMENTS, MANUAL, NOON, SEALED, Sandbox, TEXT_STAMP, barred, envelope,
    event, feed, fragment_event, fragment_page, keys, page, run_for, run_logged, sealed, texts,
};

const ASSEMBLED: &str = "title\nalice:hello\nbob:world";
const FORWARDS: &str = r#"{"messages":[{"messageId":"m1","forwarded":[{"sender":"alice","text":"hello"},{"sender":"bob","text":"world"}]},{"messageId":"m3","forwarded":[{"sender":"carol","text":"third"}]},{"messageId":"m4","forwarded":[]}]}"#;
const HUGE: u64 = 9_000_000_000_000_000_000;
const LONG: u64 = 20_000;
const LONG_BACKOFF: u64 = 60_000;
const SHORT: u64 = 2_500;
const SHORT_CHILD: u64 = 100;

#[tokio::test]
async fn event_fragments_are_assembled_from_forwards() {
    let sandbox = Sandbox::new("ingest-forward-event").unwrap();
    sandbox
        .write("listen.txt", &feed(&[fragment_event("m1")]))
        .unwrap();
    sandbox.write("lookup.json", FORWARDS).unwrap();
    let config = sandbox.config(&json!({})).unwrap();
    let (code, logs) = run_logged(&config, SHORT).await.unwrap();
    assert_eq!(code, 0);
    assert_eq!(texts(&sandbox.database()).unwrap(), [ASSEMBLED]);
    assert_eq!(
        sandbox.read("lookup-args.txt").unwrap().trim(),
        "lookup --msg-ids=m1"
    );
    assert!(logs.contains("forwards looked up"));
    assert!(!logs.contains("alice"));
}

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
async fn failed_decrypt_keeps_ciphertext() {
    for (name, mode) in [
        ("ingest-decrypt-fail", "fail"),
        ("ingest-decrypt-junk", "junk"),
    ] {
        let sandbox = Sandbox::new(name).unwrap();
        sandbox.write("listen.txt", &feed(&[sealed("m1")])).unwrap();
        let config = sandbox
            .config(&json!({"dws": {"env": {"FAKE_DECRYPT": mode}}}))
            .unwrap();
        let (code, logs) = run_logged(&config, SHORT).await.unwrap();
        assert_eq!(code, 0);
        assert_eq!(texts(&sandbox.database()).unwrap(), [SEALED]);
        assert!(logs.contains("event decrypt failed"));
        assert!(!logs.contains(SEALED));
    }
}

#[tokio::test]
async fn failed_lookup_keeps_fragments() {
    for (name, mode) in [
        ("ingest-lookup-fail", "fail"),
        ("ingest-lookup-junk", "junk"),
    ] {
        let sandbox = Sandbox::new(name).unwrap();
        sandbox
            .write("listen.txt", &feed(&[fragment_event("m1")]))
            .unwrap();
        let config = sandbox
            .config(&json!({"dws": {"env": {"FAKE_LOOKUP": mode}}}))
            .unwrap();
        assert_eq!(run_for(&config, SHORT).await.unwrap(), 0);
        assert_eq!(texts(&sandbox.database()).unwrap(), [FRAGMENTS]);
    }
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
async fn missing_decrypt_binary_keeps_ciphertext() {
    let sandbox = Sandbox::new("ingest-no-decrypt").unwrap();
    sandbox.write("listen.txt", &feed(&[sealed("m1")])).unwrap();
    let config = sandbox
        .config(&json!({"dws": {"decrypt": {"bin": "/nonexistent/dws"}}}))
        .unwrap();
    assert_eq!(run_for(&config, SHORT).await.unwrap(), 0);
    assert_eq!(texts(&sandbox.database()).unwrap(), [SEALED]);
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
async fn missing_lookup_binary_keeps_fragments() {
    let sandbox = Sandbox::new("ingest-no-lookup").unwrap();
    sandbox
        .write("listen.txt", &feed(&[fragment_event("m1")]))
        .unwrap();
    let config = sandbox
        .config(&json!({"dws": {"lookup": {"bin": "/nonexistent/dws"}}}))
        .unwrap();
    assert_eq!(run_for(&config, SHORT).await.unwrap(), 0);
    assert_eq!(texts(&sandbox.database()).unwrap(), [FRAGMENTS]);
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
async fn ordinary_text_with_bars_is_not_decrypted() {
    let sandbox = Sandbox::new("ingest-bars").unwrap();
    sandbox.write("listen.txt", &feed(&[barred("m1")])).unwrap();
    let config = sandbox.config(&json!({})).unwrap();
    assert_eq!(run_for(&config, SHORT).await.unwrap(), 0);
    assert_eq!(texts(&sandbox.database()).unwrap(), ["a || b"]);
    let missing = sandbox.read("decrypt-args.txt").unwrap_err();
    assert_eq!(missing.kind(), ErrorKind::NotFound);
}

#[tokio::test]
async fn page_fragments_are_looked_up_in_batches() {
    let sandbox = Sandbox::new("ingest-forward-page").unwrap();
    sandbox
        .write("pull.json", &fragment_page(&["m1", "m2", "m3", "m4", "p5"]))
        .unwrap();
    sandbox.write("lookup.json", FORWARDS).unwrap();
    let config = sandbox.config(&json!({})).unwrap();
    assert_eq!(run_for(&config, SHORT).await.unwrap(), 0);
    assert_eq!(
        texts(&sandbox.database()).unwrap(),
        [
            ASSEMBLED,
            FRAGMENTS,
            "title\ncarol:third",
            FRAGMENTS,
            "plain"
        ]
    );
    let calls = sandbox.read("lookup-args.txt").unwrap();
    let lines: Vec<&str> = calls.lines().collect();
    assert_eq!(lines, ["lookup --msg-ids=m1,m2", "lookup --msg-ids=m3,m4"]);
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
async fn sealed_event_is_decrypted_before_storing() {
    let sandbox = Sandbox::new("ingest-decrypt").unwrap();
    let lines = [sealed("m1"), event("m2")];
    sandbox.write("listen.txt", &feed(&lines)).unwrap();
    let config = sandbox.config(&json!({})).unwrap();
    let (code, logs) = run_logged(&config, SHORT).await.unwrap();
    assert_eq!(code, 0);
    assert_eq!(texts(&sandbox.database()).unwrap(), ["CLEARTEXT", "hello"]);
    let args = sandbox.read("decrypt-args.txt").unwrap();
    assert_eq!(args.trim(), format!("decrypt --text={SEALED}"));
    assert!(logs.contains("event decrypted"));
    assert!(!logs.contains(SEALED));
    assert!(!logs.contains("CLEARTEXT"));
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
async fn stop_during_decrypt_ends_run() {
    let sandbox = Sandbox::new("ingest-decrypt-stop").unwrap();
    sandbox.write("listen.txt", &feed(&[sealed("m1")])).unwrap();
    let config = sandbox
        .config(&json!({"dws": {"env": {"FAKE_DECRYPT": "slow"}}}))
        .unwrap();
    assert_eq!(run_for(&config, SHORT).await.unwrap(), 0);
    assert_eq!(keys(&sandbox.database()).unwrap(), Vec::<String>::new());
}

#[tokio::test]
async fn stop_during_lookup_ends_run() {
    let sandbox = Sandbox::new("ingest-lookup-stop").unwrap();
    sandbox
        .write("listen.txt", &feed(&[fragment_event("m1")]))
        .unwrap();
    let config = sandbox
        .config(&json!({"dws": {"env": {"FAKE_LOOKUP": "slow"}}}))
        .unwrap();
    assert_eq!(run_for(&config, SHORT).await.unwrap(), 0);
    assert_eq!(keys(&sandbox.database()).unwrap(), Vec::<String>::new());
}

#[tokio::test]
async fn stop_during_page_lookup_ends_run() {
    let sandbox = Sandbox::new("ingest-page-lookup-stop").unwrap();
    sandbox.write("pull.json", &fragment_page(&["m1"])).unwrap();
    let config = sandbox
        .config(&json!({"dws": {"env": {"FAKE_LOOKUP": "slow"}}}))
        .unwrap();
    assert_eq!(run_for(&config, SHORT).await.unwrap(), 0);
    assert_eq!(keys(&sandbox.database()).unwrap(), Vec::<String>::new());
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
