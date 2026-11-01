use domain::review::Ruling;
use serde_json::{Value, json};

use super::rig::{
    ALERTS, BATCHES, KEY, LATER, LINES, NOTHING, PLACEMENTS, QUICK, SECOND, Seed, THIRD, calls,
    decided, input_of, logged, one, questions_of, rule, seed, seed_from, start, table, two,
};
use crate::support::{Fallible, Sandbox};

const BACKOFF: u64 = 3_600_000;
const QUIET: &str = "0.1";
const SHORT_LOOKBACK: u64 = 1_000;
const TWO: u32 = 2;

#[tokio::test]
async fn empty_batches_close_while_the_previous_one_waits() {
    let sandbox = Sandbox::new("batch-overlap-empty").unwrap();
    seed(&sandbox, &three()).unwrap();
    let manual = json!({"limits": {"batch": TWO}, "review": {"mode": "manual"}});
    start(&sandbox, &manual, QUICK).await.unwrap();
    for key in [SECOND, THIRD] {
        let approved = rule(&sandbox, &Ruling::approve(key)).unwrap();
        assert_eq!(approved.as_deref(), Some("approved"));
    }
    let held = json!({
        "limits": {"batch": TWO, "retry_after_ms": BACKOFF},
        "review": {"mode": "manual"},
        "stub": {"decide": {"mode": "fail"}},
    });
    start(&sandbox, &held, QUICK).await.unwrap();
    let rejected = rule(&sandbox, &Ruling::reject(KEY, "noise")).unwrap();
    assert_eq!(rejected.as_deref(), Some("rejected"));
    start(&sandbox, &held, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, BATCHES).unwrap(),
        ["1|ready|1|http:500", "2|sent|0|"]
    );
    assert_eq!(calls(&sandbox, "decide").unwrap(), 1);
}

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
    let placed = table(&sandbox, PLACEMENTS).unwrap();
    assert_eq!(placed.len(), 3);
    let topic = |index: usize| {
        let row = placed.get(index)?;
        row.split('|').nth(1)
    };
    assert_eq!(topic(0), topic(2));
    assert_ne!(topic(0), topic(1));
    assert!(placed.iter().all(|row| row.ends_with("|0.95")));
}

#[tokio::test]
async fn failed_batches_do_not_block_the_next_one() {
    let sandbox = Sandbox::new("batch-overlap-failed").unwrap();
    seed(&sandbox, &three()).unwrap();
    let patch = json!({"limits": {"batch": TWO}, "stub": {"decide": {"mode": "fail"}}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(calls(&sandbox, "decide").unwrap(), 4);
    assert_eq!(
        table(&sandbox, BATCHES).unwrap(),
        ["1|failed|2|http:500", "2|failed|2|http:500"]
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
async fn lookback_continues_backlog_after_restart() {
    let sandbox = Sandbox::new("batch-lookback-restart").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"limits": {"lookback_ms": SHORT_LOOKBACK}});
    start(&sandbox, &json!({}), QUICK).await.unwrap();
    seed_from(&sandbox, &[(SECOND, "u1", "c1", "still down")], LATER).unwrap();
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(calls(&sandbox, "decide").unwrap(), 2);
    assert_eq!(
        table(&sandbox, LINES).unwrap(),
        [
            format!("{KEY}|approved||1"),
            format!("{SECOND}|approved||2")
        ]
    );
}

#[tokio::test]
async fn lookback_reanchors_after_disable() {
    let sandbox = Sandbox::new("batch-lookback-disable").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"limits": {"lookback_ms": SHORT_LOOKBACK}});
    start(&sandbox, &json!({}), QUICK).await.unwrap();
    seed_from(&sandbox, &[(SECOND, "u1", "c1", "still down")], LATER).unwrap();
    start(&sandbox, &json!({"mode": "disabled"}), QUICK)
        .await
        .unwrap();
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(calls(&sandbox, "decide").unwrap(), 1);
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
            format!("{KEY}|T1|0.95"),
            format!("{SECOND}|T1|0.95"),
            format!("{THIRD}|T1|0.95"),
        ]
    );
}

#[tokio::test]
async fn next_batch_is_reviewed_while_the_previous_one_is_stuck() {
    let sandbox = Sandbox::new("batch-overlap-review").unwrap();
    seed(&sandbox, &three()).unwrap();
    let manual = json!({"limits": {"batch": TWO}, "review": {"mode": "manual"}});
    start(&sandbox, &manual, QUICK).await.unwrap();
    for key in [SECOND, THIRD] {
        let approved = rule(&sandbox, &Ruling::approve(key)).unwrap();
        assert_eq!(approved.as_deref(), Some("approved"));
    }
    let stuck = json!({
        "limits": {"batch": TWO},
        "review": {"mode": "manual"},
        "stub": {"decide": {"mode": "slow"}},
    });
    start(&sandbox, &stuck, QUICK).await.unwrap();
    assert_eq!(table(&sandbox, BATCHES).unwrap(), ["1|ready|0|"]);
    start(&sandbox, &stuck, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, LINES).unwrap(),
        [
            format!("{KEY}|pending||2"),
            format!("{SECOND}|approved||1"),
            format!("{THIRD}|approved||1"),
        ]
    );
    assert_eq!(table(&sandbox, BATCHES).unwrap(), ["1|ready|0|"]);
}

#[tokio::test]
async fn next_batch_sees_topics_from_the_previous_one() {
    let sandbox = stalled("batch-overlap-topic").await.unwrap();
    let resume = json!({
        "limits": {"batch": TWO},
        "stub": {"decide": {"p": QUIET, "topics": "t1-0"}},
    });
    start(&sandbox, &resume, QUICK).await.unwrap();
    assert_eq!(calls(&sandbox, "decide").unwrap(), 3);
    assert_eq!(
        table(&sandbox, BATCHES).unwrap(),
        ["1|sent|0|", "2|sent|0|"]
    );
    let body = decided(&sandbox).unwrap();
    assert_eq!(questions_of(&body), ["urgent_1", "topic_1"]);
    assert_eq!(
        body.pointer("/questions/1/choices/0/value"),
        Some(&json!("t1-0"))
    );
    assert_eq!(
        table(&sandbox, PLACEMENTS).unwrap(),
        [
            format!("{KEY}|t1-0|0.1"),
            format!("{SECOND}|t1-0|0.1"),
            format!("{THIRD}|t1-0|0.1"),
        ]
    );
}

#[tokio::test]
async fn next_batch_waits_for_the_previous_decision_before_sealing() {
    let sandbox = stalled("batch-overlap-seal").await.unwrap();
    assert_eq!(calls(&sandbox, "decide").unwrap(), 1);
    assert_eq!(table(&sandbox, BATCHES).unwrap(), ["1|ready|1|http:500"]);
    assert_eq!(
        table(&sandbox, LINES).unwrap(),
        [
            format!("{KEY}|approved||2"),
            format!("{SECOND}|approved||1"),
            format!("{THIRD}|approved||1"),
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
    let patch = json!({"stub": {"decide": {"ps": "0.1 0.9"}}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, PLACEMENTS).unwrap(),
        [format!("{KEY}|T1|0.1"), format!("{SECOND}|T1|0.9")]
    );
    assert_eq!(
        table(&sandbox, ALERTS).unwrap(),
        ["t1-0@1|sent|0|ANALYSIS|"]
    );
}

async fn stalled(name: &str) -> Fallible<Sandbox> {
    let sandbox = Sandbox::new(name)?;
    seed(&sandbox, &three())?;
    let patch = json!({
        "limits": {"batch": TWO, "retry_after_ms": BACKOFF},
        "stub": {"decide": {"mode": "fail", "p": QUIET}},
    });
    start(&sandbox, &patch, QUICK).await?;
    Ok(sandbox)
}

const fn three() -> [Seed<'static>; 3] {
    [
        (KEY, "u1", "c2", "server down"),
        (SECOND, "u1", "c1", "still down"),
        (THIRD, "u1", "c1", "again"),
    ]
}
