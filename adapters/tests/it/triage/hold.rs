use serde_json::json;

use super::rig::{PICK, calls, draw, logged, one, seed, start};
use crate::support::Sandbox;

const BRIEF_RETRY: u64 = 300;
const FEW: usize = 14;
const NONE: u32 = 0;
const QUICK_HOLD: u64 = 1_500;

#[tokio::test]
async fn decision_quota_is_logged_once() {
    let sandbox = Sandbox::new("hold-decisions").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"limits": {"max_decisions_per_hour": NONE}});
    let logs = logged(&sandbox, &patch, QUICK_HOLD).await.unwrap();
    assert_eq!(logs.matches("decision quota reached").count(), 1);
}

#[tokio::test]
async fn handoff_hold_is_logged_once() {
    let sandbox = Sandbox::new("hold-handoffs").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"limits": {"max_handoffs_per_hour": NONE}});
    let logs = logged(&sandbox, &patch, QUICK_HOLD).await.unwrap();
    assert_eq!(logs.matches("handoff held").count(), 1);
    assert!(logs.contains("quota"));
}

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
async fn review_wait_is_logged_once() {
    let sandbox = Sandbox::new("hold-review").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"review": {"mode": "manual"}});
    start(&sandbox, &patch, QUICK_HOLD).await.unwrap();
    assert_eq!(draw(&sandbox, PICK).unwrap(), "1|1");
    let logs = logged(&sandbox, &patch, QUICK_HOLD).await.unwrap();
    assert_eq!(logs.matches("batch waiting").count(), 1);
    assert!(logs.contains("review@1"));
}
