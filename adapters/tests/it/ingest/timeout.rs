use serde_json::json;

use crate::feeds::{event, feed, fragment_event, sealed};
use crate::support::{FRAGMENTS, SEALED, Sandbox, keys, run_logged, texts};

const BRIEF: u64 = 200;
const SHORT: u64 = 4_000;

#[tokio::test]
async fn slow_decrypt_times_out_and_keeps_ciphertext() {
    let sandbox = Sandbox::new("ingest-timeout-decrypt").unwrap();
    sandbox.write("listen.txt", &feed(&[sealed("m1")])).unwrap();
    let config = sandbox
        .config(&json!({"dws": {"env": {"FAKE_DECRYPT": "slow"}, "timeout_ms": BRIEF}}))
        .unwrap();
    let (code, logs) = run_logged(&config, SHORT).await.unwrap();
    assert_eq!(code, 0);
    assert_eq!(texts(&sandbox.database()).unwrap(), [SEALED]);
    assert!(logs.contains("message decrypt failed"));
}

#[tokio::test]
async fn slow_lookup_times_out_and_keeps_fragments() {
    let sandbox = Sandbox::new("ingest-timeout-lookup").unwrap();
    sandbox
        .write("listen.txt", &feed(&[fragment_event("m1")]))
        .unwrap();
    let config = sandbox
        .config(&json!({"dws": {"env": {"FAKE_LOOKUP": "slow"}, "timeout_ms": BRIEF}}))
        .unwrap();
    let (code, _logs) = run_logged(&config, SHORT).await.unwrap();
    assert_eq!(code, 0);
    assert_eq!(texts(&sandbox.database()).unwrap(), [FRAGMENTS]);
}

#[tokio::test]
async fn slow_pull_times_out_and_listening_continues() {
    let sandbox = Sandbox::new("ingest-timeout-pull").unwrap();
    sandbox.write("listen.txt", &feed(&[event("m1")])).unwrap();
    let config = sandbox
        .config(&json!({"dws": {"env": {"FAKE_PULL": "slow"}, "timeout_ms": BRIEF}}))
        .unwrap();
    let (code, logs) = run_logged(&config, SHORT).await.unwrap();
    assert_eq!(code, 0);
    assert_eq!(keys(&sandbox.database()).unwrap(), ["dingtalk:acct:m1"]);
    assert!(logs.contains("TimedOut") || logs.contains("timed out"));
}
