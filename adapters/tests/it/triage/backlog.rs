use serde_json::json;

use super::rig::{
    ALERTS, BATCHES, KEY, LINES, NOTHING, PLACEMENTS, QUICK, SECOND, THIRD, calls, now, seed,
    seed_stamped, start, table,
};
use crate::support::Sandbox;

const FIFTH: &str = "dingtalk:acct:m5";
const FOURTH: &str = "dingtalk:acct:m4";
const QUIET: &str = "0.1";
const RECENT: u64 = 600_000;
const SHORT_LOOKBACK: u64 = 1_000;
const TWO: u32 = 2;

#[tokio::test]
async fn backlog_beyond_the_lookback_is_decided_without_handoff() {
    let sandbox = Sandbox::new("backlog-stale").unwrap();
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
    assert_eq!(calls(&sandbox, "decide").unwrap(), 2);
    assert_eq!(table(&sandbox, PLACEMENTS).unwrap().len(), 3);
    assert_eq!(table(&sandbox, ALERTS).unwrap(), NOTHING);
}

#[tokio::test]
async fn old_and_recent_lines_are_batched_oldest_first() {
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
    let patch = json!({
        "limits": {"batch": TWO, "lookback_ms": RECENT},
        "stub": {"decide": {"p": QUIET}},
    });
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, LINES).unwrap(),
        [
            format!("{KEY}|approved||1"),
            format!("{SECOND}|approved||1"),
            format!("{THIRD}|approved||2"),
            format!("{FOURTH}|approved||2"),
            format!("{FIFTH}|approved||3"),
        ]
    );
    assert_eq!(
        table(&sandbox, BATCHES).unwrap(),
        ["1|sent|0|", "2|sent|0|", "3|sent|0|"]
    );
}
