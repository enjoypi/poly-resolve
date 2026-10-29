use domain::review::{Rule, Ruling};
use serde_json::{Value, json};

use super::rig::{
    BATCHES, KEY, LINES, NOTHING, PLACEMENTS, QUICK, SECOND, THIRD, calls, decided, input_of,
    questions_of, rule, seed, seed_from, start, table, two,
};
use crate::support::Sandbox;

const LATER: u64 = 60_000;
const ONE: u32 = 1;
const SECRET: &str = "abc-secret-9";

#[tokio::test]
async fn leaked_text_is_masked_again_and_reviewed_before_sending() {
    let sandbox = Sandbox::new("review-leak").unwrap();
    let text = format!("token {SECRET} here");
    let rows = [
        (KEY, "u1", "c1", text.as_str()),
        (SECOND, "u1", "c1", text.as_str()),
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
    let ruling = Ruling::reject(KEY, "alice", "leak").learned(&learned);
    assert_eq!(rule(&sandbox, &ruling).unwrap().as_deref(), Some("leaked"));
    assert_eq!(
        table(
            &sandbox,
            "SELECT pattern || '|' || placeholder || '|' || reviewer AS item FROM learned_rules"
        )
        .unwrap(),
        [r"abc\-secret\-9|[x]|alice"]
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
    let approved = rule(&sandbox, &Ruling::approve(KEY, "alice")).unwrap();
    assert_eq!(approved.as_deref(), Some("approved"));
    start(&sandbox, &patch, QUICK).await.unwrap();
    let input = input_of(&decided(&sandbox).unwrap()).unwrap();
    assert_eq!(
        input.pointer("/0/lines/0/text").and_then(Value::as_str),
        Some("token [x] here")
    );
    assert!(!sandbox.read("decide-stdin.txt").unwrap().contains(SECRET));
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
    let approved = rule(&sandbox, &Ruling::approve(KEY, "alice")).unwrap();
    assert_eq!(approved.as_deref(), Some("approved"));
    start(&sandbox, &manual(), QUICK).await.unwrap();
    assert_eq!(calls(&sandbox, "decide").unwrap(), 0);
    let rejected = rule(&sandbox, &Ruling::reject(SECOND, "alice", "noise")).unwrap();
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
        [format!("{KEY}|t1-0|0.95")]
    );
}

#[tokio::test]
async fn rejected_batches_close_without_a_request() {
    let sandbox = Sandbox::new("review-closed").unwrap();
    seed(&sandbox, &[(KEY, "u1", "c1", "server down")]).unwrap();
    start(&sandbox, &manual(), QUICK).await.unwrap();
    let rejected = rule(&sandbox, &Ruling::reject(KEY, "alice", "noise")).unwrap();
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
