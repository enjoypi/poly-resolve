use serde_json::json;

use super::rig::{
    ALERTS, BATCHES, KEY, LINES, NOTHING, QUICK, SECOND, THIRD, calls, logged, now, seed,
    seed_stamped, start, table,
};
use crate::support::Sandbox;

const FIFTH: &str = "dingtalk:acct:m5";
const FOURTH: &str = "dingtalk:acct:m4";
const RECENT: u64 = 600_000;
const SHORT_LOOKBACK: u64 = 1_000;
const TWO: u32 = 2;

#[tokio::test]
async fn backlog_beyond_the_cap_is_left_out() {
    let sandbox = Sandbox::new("backlog-cap").unwrap();
    start(&sandbox, &json!({}), QUICK).await.unwrap();
    seed(&sandbox, &[(KEY, "u1", "c1", "server down")]).unwrap();
    let patch =
        json!({"limits": {"backlog_max_ms": SHORT_LOOKBACK, "lookback_ms": SHORT_LOOKBACK}});
    let logs = logged(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(logs.matches("left out").count(), 1);
    assert_eq!(table(&sandbox, LINES).unwrap(), NOTHING);
    assert_eq!(calls(&sandbox, "decide").unwrap(), 0);
}

#[tokio::test]
async fn backlog_is_batched_oldest_first_without_handoffs() {
    let sandbox = Sandbox::new("backlog-oldest").unwrap();
    start(&sandbox, &json!({}), QUICK).await.unwrap();
    let rows = [
        (KEY, "u1", "c1", "server down"),
        (SECOND, "u1", "c1", "still down"),
        (THIRD, "u1", "c1", "again"),
    ];
    seed(&sandbox, &rows).unwrap();
    let patch = json!({"limits": {"batch": TWO, "lookback_ms": SHORT_LOOKBACK}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, LINES).unwrap(),
        [
            format!("{KEY}|approved||1"),
            format!("{SECOND}|approved||1"),
            format!("{THIRD}|approved||2"),
        ]
    );
    assert_eq!(
        table(&sandbox, BATCHES).unwrap(),
        ["1|sent|0|", "2|sent|0|"]
    );
    assert_eq!(table(&sandbox, ALERTS).unwrap(), NOTHING);
}

#[tokio::test]
async fn recent_lines_are_batched_before_the_backlog() {
    let sandbox = Sandbox::new("backlog-recent").unwrap();
    start(&sandbox, &json!({}), QUICK).await.unwrap();
    let old = [
        (KEY, "u1", "c1", "server down"),
        (SECOND, "u1", "c1", "still down"),
    ];
    seed(&sandbox, &old).unwrap();
    let stamp = now().unwrap();
    let recent = [
        ((THIRD, "u1", "c2", "db slow"), stamp.saturating_sub(3)),
        ((FOURTH, "u1", "c2", "db slower"), stamp.saturating_sub(2)),
        ((FIFTH, "u1", "c2", "db stuck"), stamp.saturating_sub(1)),
    ];
    seed_stamped(&sandbox, &recent).unwrap();
    let patch = json!({"limits": {"batch": TWO, "lookback_ms": RECENT}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, LINES).unwrap(),
        [
            format!("{KEY}|approved||2"),
            format!("{SECOND}|approved||3"),
            format!("{THIRD}|approved||1"),
            format!("{FOURTH}|approved||1"),
            format!("{FIFTH}|approved||2"),
        ]
    );
    assert_eq!(
        table(&sandbox, BATCHES).unwrap(),
        ["1|sent|0|", "2|sent|0|", "3|sent|0|"]
    );
}
