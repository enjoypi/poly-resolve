use domain::review::{Rule, Ruling};
use serde_json::{Value, json};

use super::rig::{
    BATCHES, KEY, LINES, NOTHING, PLACEMENTS, QUICK, SECOND, THIRD, calls, decided, input_of,
    logged, questions_of, rule, seed, seed_from, start, table, two,
};
use crate::support::Sandbox;

const BACKOFF: u64 = 3_600_000;
const LATER: u64 = 60_000;
const ONE: u32 = 1;
const PAYLOAD: &str = "SELECT payload AS item FROM batches ORDER BY id";
const SECRET: &str = "abc-secret-9";

#[tokio::test]
async fn answered_batches_replay_without_repacking() {
    let sandbox = Sandbox::new("review-replay").unwrap();
    seed(&sandbox, &[(KEY, "u1", "c1", "server down")]).unwrap();
    sandbox
        .seed("CREATE TRIGGER refuse BEFORE INSERT ON placements BEGIN SELECT RAISE(ABORT, 'refused'); END;")
        .unwrap();
    start(&sandbox, &json!({}), QUICK).await.unwrap();
    assert_eq!(table(&sandbox, BATCHES).unwrap(), ["1|ready|1|"]);
    let payload = table(&sandbox, PAYLOAD).unwrap();
    sandbox
        .seed("DROP TRIGGER refuse; INSERT INTO learned_rules (pattern, placeholder, source_key, reviewer, created_at_ms) VALUES ('server', '[s]', 'k', '', 1);")
        .unwrap();
    start(&sandbox, &json!({}), QUICK).await.unwrap();
    assert_eq!(calls(&sandbox, "decide").unwrap(), 1);
    assert_eq!(table(&sandbox, BATCHES).unwrap(), ["1|sent|0|"]);
    assert_eq!(table(&sandbox, PAYLOAD).unwrap(), payload);
}

#[tokio::test]
async fn empty_batches_do_not_spend_the_decision_quota() {
    let sandbox = Sandbox::new("review-quota").unwrap();
    let patch = json!({
        "limits": {"max_decisions_per_hour": ONE},
        "review": {"mode": "manual"},
    });
    seed(&sandbox, &[(KEY, "u1", "c1", "server down")]).unwrap();
    start(&sandbox, &patch, QUICK).await.unwrap();
    let rejected = rule(&sandbox, &Ruling::reject(KEY, "noise")).unwrap();
    assert_eq!(rejected.as_deref(), Some("rejected"));
    seed_from(&sandbox, &[(THIRD, "u1", "c1", "next")], LATER).unwrap();
    start(&sandbox, &patch, QUICK).await.unwrap();
    let approved = rule(&sandbox, &Ruling::approve(THIRD)).unwrap();
    assert_eq!(approved.as_deref(), Some("approved"));
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(calls(&sandbox, "decide").unwrap(), 1);
    assert_eq!(
        table(&sandbox, BATCHES).unwrap(),
        ["1|sent|0|", "2|sent|0|"]
    );
}

#[tokio::test]
async fn invalid_learned_rules_hold_a_waiting_batch() {
    let sandbox = Sandbox::new("review-repack-broken").unwrap();
    seed(&sandbox, &[(KEY, "u1", "c1", "server down")]).unwrap();
    let held = json!({
        "limits": {"retry_after_ms": BACKOFF},
        "stub": {"decide": {"mode": "fail"}},
    });
    start(&sandbox, &held, QUICK).await.unwrap();
    assert_eq!(table(&sandbox, BATCHES).unwrap(), ["1|ready|1|http:500"]);
    sandbox
        .seed("INSERT INTO learned_rules (pattern, placeholder, source_key, reviewer, created_at_ms) VALUES ('(', '[x]', 'k', '', 1);")
        .unwrap();
    let resume = json!({"stub": {"decide": {}}});
    let logs = logged(&sandbox, &resume, QUICK).await.unwrap();
    assert!(logs.contains("batch repack failed"));
    assert_eq!(calls(&sandbox, "decide").unwrap(), 1);
    assert_eq!(table(&sandbox, BATCHES).unwrap(), ["1|ready|1|http:500"]);
}

#[tokio::test]
async fn leaked_text_is_masked_again_and_reviewed_before_sending() {
    let sandbox = Sandbox::new("review-leak").unwrap();
    let text = format!("token {SECRET} here");
    let rows = [
        (SECOND, "u1", "c1", text.as_str()),
        (KEY, "u1", "c1", text.as_str()),
    ];
    seed(&sandbox, &rows).unwrap();
    let patch = json!({
        "limits": {"batch": ONE},
        "review": {"mode": "manual"},
    });
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, LINES).unwrap(),
        [format!("{KEY}|pending||1"), format!("{SECOND}|queued||0")]
    );
    let learned = Rule::new(r"abc\-secret\-9".to_owned(), "[x]".to_owned());
    let ruling = Ruling::reject(KEY, "leak").learned(&learned);
    assert_eq!(rule(&sandbox, &ruling).unwrap().as_deref(), Some("leaked"));
    assert_eq!(
        table(
            &sandbox,
            "SELECT pattern || '|' || placeholder AS item FROM learned_rules"
        )
        .unwrap(),
        [r"abc\-secret\-9|[x]"]
    );
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, LINES).unwrap(),
        [
            format!("{KEY}|pending|leak|1"),
            format!("{SECOND}|queued||0")
        ]
    );
    assert_eq!(
        table(&sandbox, "SELECT text AS item FROM lines ORDER BY key").unwrap(),
        ["token [x] here", "token [x] here"]
    );
    assert_eq!(calls(&sandbox, "decide").unwrap(), 0);
    let approved = rule(&sandbox, &Ruling::approve(KEY)).unwrap();
    assert_eq!(approved.as_deref(), Some("approved"));
    start(&sandbox, &patch, QUICK).await.unwrap();
    let input = input_of(&decided(&sandbox).unwrap()).unwrap();
    assert_eq!(
        input.pointer("/0/lines/0/text").and_then(Value::as_str),
        Some("token [x] here")
    );
    assert!(!sandbox.read("decide-body.txt").unwrap().contains(SECRET));
}

#[tokio::test]
async fn learned_rules_reach_a_batch_waiting_to_be_sent() {
    let sandbox = Sandbox::new("review-repack").unwrap();
    let text = format!("token {SECRET} here");
    let rows = [
        (SECOND, "u1", "c2", text.as_str()),
        (KEY, "u1", "c1", text.as_str()),
    ];
    seed(&sandbox, &rows).unwrap();
    let held = json!({
        "limits": {"batch": ONE, "retry_after_ms": BACKOFF},
        "review": {"mode": "manual"},
        "stub": {"decide": {"mode": "fail"}},
    });
    start(&sandbox, &held, QUICK).await.unwrap();
    let approved = rule(&sandbox, &Ruling::approve(KEY)).unwrap();
    assert_eq!(approved.as_deref(), Some("approved"));
    start(&sandbox, &held, QUICK).await.unwrap();
    assert_eq!(table(&sandbox, BATCHES).unwrap(), ["1|ready|1|http:500"]);
    assert!(sandbox.read("decide-body.txt").unwrap().contains(SECRET));
    let learned = Rule::new(r"abc\-secret\-9".to_owned(), "[x]".to_owned());
    let ruling = Ruling::reject(SECOND, "leak").learned(&learned);
    assert_eq!(rule(&sandbox, &ruling).unwrap().as_deref(), Some("leaked"));
    let resume =
        json!({"limits": {"batch": ONE}, "review": {"mode": "manual"}, "stub": {"decide": {}}});
    start(&sandbox, &resume, QUICK).await.unwrap();
    assert_eq!(calls(&sandbox, "decide").unwrap(), 2);
    assert_eq!(table(&sandbox, BATCHES).unwrap(), ["1|sent|0|"]);
    assert!(!sandbox.read("decide-body.txt").unwrap().contains(SECRET));
    assert!(!table(&sandbox, PAYLOAD).unwrap().concat().contains(SECRET));
}

fn manual() -> Value {
    json!({"review": {"mode": "manual"}})
}

#[tokio::test]
async fn manual_lines_wait_for_approval_and_rejected_ones_stay_home() {
    let sandbox = Sandbox::new("review-manual").unwrap();
    seed(&sandbox, &two()).unwrap();
    start(&sandbox, &manual(), QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, LINES).unwrap(),
        [format!("{KEY}|pending||1"), format!("{SECOND}|pending||1")]
    );
    assert_eq!(calls(&sandbox, "decide").unwrap(), 0);
    assert_eq!(table(&sandbox, BATCHES).unwrap(), NOTHING);
    let approved = rule(&sandbox, &Ruling::approve(KEY)).unwrap();
    assert_eq!(approved.as_deref(), Some("approved"));
    start(&sandbox, &manual(), QUICK).await.unwrap();
    assert_eq!(calls(&sandbox, "decide").unwrap(), 0);
    let rejected = rule(&sandbox, &Ruling::reject(SECOND, "noise")).unwrap();
    assert_eq!(rejected.as_deref(), Some("rejected"));
    start(&sandbox, &manual(), QUICK).await.unwrap();
    assert_eq!(calls(&sandbox, "decide").unwrap(), 1);
    assert_eq!(questions_of(&decided(&sandbox).unwrap()), ["urgent_1"]);
    assert_eq!(
        table(&sandbox, LINES).unwrap(),
        [
            format!("{KEY}|approved||1"),
            format!("{SECOND}|rejected|noise|1")
        ]
    );
    assert_eq!(
        table(&sandbox, PLACEMENTS).unwrap(),
        [format!("{KEY}|T1|0.95")]
    );
}

#[tokio::test]
async fn rejected_batches_close_without_a_request() {
    let sandbox = Sandbox::new("review-closed").unwrap();
    seed(&sandbox, &[(KEY, "u1", "c1", "server down")]).unwrap();
    start(&sandbox, &manual(), QUICK).await.unwrap();
    let rejected = rule(&sandbox, &Ruling::reject(KEY, "noise")).unwrap();
    assert_eq!(rejected.as_deref(), Some("rejected"));
    seed_from(&sandbox, &[(THIRD, "u1", "c1", "next")], LATER).unwrap();
    start(&sandbox, &manual(), QUICK).await.unwrap();
    assert_eq!(table(&sandbox, BATCHES).unwrap(), ["1|sent|0|"]);
    assert_eq!(calls(&sandbox, "decide").unwrap(), 0);
    assert_eq!(
        table(&sandbox, LINES).unwrap(),
        [
            format!("{KEY}|rejected|noise|1"),
            format!("{THIRD}|pending||2")
        ]
    );
}
