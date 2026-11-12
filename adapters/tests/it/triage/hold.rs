use serde_json::json;

use super::rig::{calls, logged, one, seed, start, two_chats};
use crate::support::Sandbox;

const BACKOFF: u64 = 3_600_000;
const BRIEF_RETRY: u64 = 300;
const FEW: usize = 14;
const ONE: u32 = 1;
const QUICK_HOLD: u64 = 1_500;

#[tokio::test]
async fn ledger_faults_back_the_lane_off() {
    let sandbox = Sandbox::new("hold-faults").unwrap();
    seed(&sandbox, &one()).unwrap();
    start(&sandbox, &json!({"mode": "disabled"}), QUICK_HOLD)
        .await
        .unwrap();
    sandbox
        .seed("CREATE TRIGGER refuse BEFORE INSERT ON lines BEGIN SELECT RAISE(ABORT, 'refused'); END;")
        .unwrap();
    let logs = logged(&sandbox, &json!({}), QUICK_HOLD).await.unwrap();
    assert!(logs.contains("lane backing off"));
    let failures = logs.matches("ledger write failed").count();
    assert!((2..FEW).contains(&failures), "{failures}");
}

#[tokio::test]
async fn retries_are_logged_with_their_wait() {
    for (name, patch, message) in [
        (
            "decide",
            json!({"stub": {"decide": {"mode": "fail"}}}),
            "decision failed",
        ),
        (
            "codex",
            json!({"codex": {"env": {"FAKE_CODEX": "fail"}}}),
            "alert analysis failed",
        ),
        (
            "send",
            json!({"notify": {"env": {"FAKE_SEND": "fail"}}}),
            "alert step failed",
        ),
    ] {
        let sandbox = Sandbox::new(&format!("hold-retry-{name}")).unwrap();
        seed(&sandbox, &one()).unwrap();
        let mut merged = patch;
        if let Some(map) = merged.as_object_mut() {
            drop(map.insert("limits".to_owned(), json!({"retry_after_ms": BRIEF_RETRY})));
        }
        let logs = logged(&sandbox, &merged, QUICK_HOLD).await.unwrap();
        let line = logs
            .lines()
            .find(|line| line.contains(message))
            .unwrap_or_default();
        assert!(line.contains("\"attempt\":1"), "{name} {line}");
        assert!(line.contains("\"retry_in_ms\":300"), "{name} {line}");
        assert!(calls(&sandbox, name).unwrap() <= 2, "{name}");
    }
}

#[tokio::test]
async fn sending_wait_is_logged_once() {
    let sandbox = Sandbox::new("hold-sending").unwrap();
    seed(&sandbox, &two_chats()).unwrap();
    let patch = json!({
        "limits": {"batch": ONE, "retry_after_ms": BACKOFF},
        "stub": {"decide": {"mode": "fail"}},
    });
    let logs = logged(&sandbox, &patch, QUICK_HOLD).await.unwrap();
    assert_eq!(logs.matches("batch waiting").count(), 1);
    assert!(logs.contains("sending@1"));
}
