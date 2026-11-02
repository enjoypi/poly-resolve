use serde_json::json;

use super::rig::{
    ALERTS, BATCHES, KEY, LINES, NOTHING, QUICK, SECOND, THIRD, calls, logged, seed, start, table,
};
use crate::support::Sandbox;

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
