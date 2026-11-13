use serde_json::json;

use super::rig::{QUICK, calls, one, seed, start, table};
use adapters::store::Archive;

use crate::support::{BUSY, Sandbox};

const BUDGET: u64 = 1;
const DOWN: &str = "INSERT INTO source_health (source, account, state, streak, ok_at_ms, down_at_ms) VALUES ('imap', 'box', 'retrying', 3, 0, 1);";
const REMINDERS: &str =
    "SELECT key || '|' || state || '|' || attempts AS item FROM reminders ORDER BY key";
const SOON: u64 = 1;

#[tokio::test]
async fn budget_overrun_is_reported_once_a_day() {
    let sandbox = Sandbox::new("remind-budget").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"limits": {"daily_tokens": BUDGET}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    start(&sandbox, &patch, QUICK).await.unwrap();
    let found = table(&sandbox, REMINDERS).unwrap();
    assert_eq!(found.len(), 1);
    assert!(found.first().unwrap().starts_with("budget:"));
    assert!(found.first().unwrap().ends_with("|sent|1"));
}

#[tokio::test]
async fn failed_reminders_are_retried_until_exhausted() {
    let sandbox = Sandbox::new("remind-failed").unwrap();
    drop(Archive::open(&sandbox.database(), BUSY).unwrap());
    sandbox.seed(DOWN).unwrap();
    let patch = json!({
        "limits": {"down_after_ms": SOON},
        "notify": {"env": {"FAKE_SEND": "fail"}},
    });
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, REMINDERS).unwrap(),
        ["down:imap:box:1|failed|2"]
    );
}

#[tokio::test]
async fn interrupted_reminders_are_released() {
    let sandbox = Sandbox::new("remind-stop").unwrap();
    drop(Archive::open(&sandbox.database(), BUSY).unwrap());
    sandbox.seed(DOWN).unwrap();
    let patch = json!({
        "limits": {"down_after_ms": SOON},
        "notify": {"env": {"FAKE_SEND": "slow"}},
    });
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, REMINDERS).unwrap(),
        ["down:imap:box:1|pending|0"]
    );
}

#[tokio::test]
async fn outages_and_recoveries_are_reported_once() {
    let sandbox = Sandbox::new("remind-down").unwrap();
    drop(Archive::open(&sandbox.database(), BUSY).unwrap());
    sandbox.seed(DOWN).unwrap();
    let patch = json!({"limits": {"down_after_ms": SOON}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        sandbox.read("send-stdin.txt").unwrap(),
        "down imap box retry 3 1970-01-01 00:00:00 UTC"
    );
    assert!(
        sandbox
            .read("send-args.txt")
            .unwrap()
            .contains("--idempotency-key=down:imap:box:1")
    );
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(calls(&sandbox, "send").unwrap(), 1);
    sandbox
        .seed("UPDATE source_health SET state = 'ok', streak = 0, ok_at_ms = 5;")
        .unwrap();
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(sandbox.read("send-stdin.txt").unwrap(), "up imap box");
    assert_eq!(
        table(&sandbox, REMINDERS).unwrap(),
        ["down:imap:box:1|sent|1", "up:imap:box:1|sent|1"]
    );
}

#[tokio::test]
async fn recent_outages_wait_for_the_grace_period() {
    let sandbox = Sandbox::new("remind-grace").unwrap();
    drop(Archive::open(&sandbox.database(), BUSY).unwrap());
    sandbox.seed(DOWN).unwrap();
    start(&sandbox, &json!({}), QUICK).await.unwrap();
    assert_eq!(calls(&sandbox, "send").unwrap(), 0);
}

#[tokio::test]
async fn unclaimable_reminders_are_not_sent() {
    let sandbox = Sandbox::new("remind-claim").unwrap();
    drop(Archive::open(&sandbox.database(), BUSY).unwrap());
    sandbox.seed(DOWN).unwrap();
    sandbox
        .seed("CREATE TRIGGER refuse BEFORE INSERT ON reminders BEGIN SELECT RAISE(ABORT, 'refused'); END;")
        .unwrap();
    let patch = json!({"limits": {"down_after_ms": SOON}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(calls(&sandbox, "send").unwrap(), 0);
}
