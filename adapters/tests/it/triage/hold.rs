use serde_json::json;

use super::rig::{logged, one, seed};
use crate::support::Sandbox;

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
async fn review_wait_is_logged_once() {
    let sandbox = Sandbox::new("hold-review").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"review": {"mode": "manual"}});
    let logs = logged(&sandbox, &patch, QUICK_HOLD).await.unwrap();
    assert_eq!(logs.matches("batch waiting").count(), 1);
    assert!(logs.contains("review@1"));
}
