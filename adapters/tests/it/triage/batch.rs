use serde_json::{Value, json};

use super::rig::{
    ALERTS, BATCHES, KEY, LINES, NOTHING, PLACEMENTS, QUICK, SECOND, THIRD, calls, decided,
    input_of, logged, questions_of, seed, start, table, two,
};
use crate::support::Sandbox;

const SHORT_LOOKBACK: u64 = 1_000;
const TWO: u32 = 2;

#[tokio::test]
async fn every_message_joins_one_batch() {
    let sandbox = Sandbox::new("batch-all").unwrap();
    let rows = [
        (KEY, "u1", "c1", "server down"),
        (SECOND, "u1", "c2", "still down"),
        (THIRD, "u1", "c1", "again"),
    ];
    seed(&sandbox, &rows).unwrap();
    start(&sandbox, &json!({}), QUICK).await.unwrap();
    assert_eq!(calls(&sandbox, "decide").unwrap(), 1);
    assert_eq!(
        table(&sandbox, LINES).unwrap(),
        [
            format!("{KEY}|approved||1"),
            format!("{SECOND}|approved||1"),
            format!("{THIRD}|approved||1"),
        ]
    );
    let body = decided(&sandbox).unwrap();
    assert_eq!(
        questions_of(&body),
        ["urgent_1", "urgent_2", "urgent_3", "topic_2"]
    );
    assert_eq!(
        table(&sandbox, PLACEMENTS).unwrap(),
        [
            format!("{KEY}|t1-0|0.95"),
            format!("{SECOND}|t1-1|0.95"),
            format!("{THIRD}|t1-0|0.95"),
        ]
    );
}

#[tokio::test]
async fn invalid_rules_fail_closed() {
    let sandbox = Sandbox::new("batch-rules").unwrap();
    seed(&sandbox, &two()).unwrap();
    let patch = json!({"redact": {"patterns": [{"pattern": "(", "placeholder": "x"}]}});
    let logs = logged(&sandbox, &patch, QUICK).await.unwrap();
    assert!(logs.contains("line unavailable"));
    assert_eq!(table(&sandbox, LINES).unwrap(), NOTHING);
    assert_eq!(calls(&sandbox, "decide").unwrap(), 0);
}

#[tokio::test]
async fn messages_before_the_lookback_are_ignored() {
    let sandbox = Sandbox::new("batch-lookback").unwrap();
    seed(&sandbox, &two()).unwrap();
    let patch = json!({"limits": {"lookback_ms": SHORT_LOOKBACK}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(table(&sandbox, LINES).unwrap(), NOTHING);
    assert_eq!(calls(&sandbox, "decide").unwrap(), 0);
}

#[tokio::test]
async fn newest_messages_are_batched_first() {
    let sandbox = Sandbox::new("batch-next").unwrap();
    let rows = [
        (KEY, "u1", "c1", "server down"),
        (SECOND, "u1", "c1", "still down"),
        (THIRD, "u1", "c1", "again"),
    ];
    seed(&sandbox, &rows).unwrap();
    let patch = json!({"limits": {"batch": TWO}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(calls(&sandbox, "decide").unwrap(), 2);
    assert_eq!(
        table(&sandbox, LINES).unwrap(),
        [
            format!("{KEY}|approved||2"),
            format!("{SECOND}|approved||1"),
            format!("{THIRD}|approved||1"),
        ]
    );
    assert_eq!(
        table(&sandbox, BATCHES).unwrap(),
        ["1|sent|0|", "2|sent|0|"]
    );
    assert_eq!(
        table(&sandbox, PLACEMENTS).unwrap(),
        [
            format!("{KEY}|t1-0|0.95"),
            format!("{SECOND}|t1-0|0.95"),
            format!("{THIRD}|t1-0|0.95"),
        ]
    );
}

#[tokio::test]
async fn payload_masks_secrets_and_the_log_stays_clean() {
    let sandbox = Sandbox::new("batch-mask").unwrap();
    seed(&sandbox, &[(KEY, "u1", "c1", "call 13812345678 now")]).unwrap();
    let logs = logged(&sandbox, &json!({}), QUICK).await.unwrap();
    let input = input_of(&decided(&sandbox).unwrap()).unwrap();
    let text = input.pointer("/0/lines/0/text").and_then(Value::as_str);
    assert_eq!(text, Some("call [num] now"));
    assert!(!logs.contains("13812345678"));
}

#[tokio::test]
async fn priorities_are_scored_per_line() {
    let sandbox = Sandbox::new("batch-priorities").unwrap();
    seed(&sandbox, &two()).unwrap();
    let patch = json!({"decide": {"env": {"FAKE_PS": "0.1 0.9"}}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, PLACEMENTS).unwrap(),
        [format!("{KEY}|t1-0|0.1"), format!("{SECOND}|t1-0|0.9")]
    );
    assert_eq!(
        table(&sandbox, ALERTS).unwrap(),
        ["t1-0@1|sent|0|ANALYSIS|"]
    );
}
