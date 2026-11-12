use serde_json::json;

use super::rig::{BATCHES, KEY, QUICK, SECOND, calls, logged, seed, start, table};
use crate::support::Sandbox;

const BACKOFF: u64 = 3_600_000;
const LEARNED: &str = "INSERT INTO learned_rules (pattern, placeholder, source_key, reviewer, created_at_ms) VALUES ('abc\\-secret\\-9', '[x]', 'k', '', 1);";
const ONE: u32 = 1;
const PAYLOAD: &str = "SELECT payload AS item FROM batches ORDER BY id";
const SECRET: &str = "abc-secret-9";

#[tokio::test]
async fn answered_batches_replay_without_repacking() {
    let sandbox = Sandbox::new("repack-replay").unwrap();
    seed(&sandbox, &[(KEY, "u1", "c1", "server down")]).unwrap();
    sandbox
        .seed("CREATE TRIGGER refuse BEFORE INSERT ON members BEGIN SELECT RAISE(ABORT, 'refused'); END;")
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
async fn invalid_stored_rules_hold_a_waiting_batch() {
    let sandbox = Sandbox::new("repack-broken").unwrap();
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
async fn stored_rules_mask_lines_still_waiting_for_screening() {
    let sandbox = Sandbox::new("repack-screening").unwrap();
    let text = format!("token {SECRET} here");
    seed(&sandbox, &[(KEY, "u1", "c1", text.as_str())]).unwrap();
    start(&sandbox, &sandbox.screen("fail"), QUICK)
        .await
        .unwrap();
    assert_eq!(
        table(&sandbox, "SELECT state || '|' || text AS item FROM lines").unwrap(),
        [format!("screening|{text}")]
    );
    sandbox.seed(LEARNED).unwrap();
    let mut resume = sandbox.screen("");
    if let Some(map) = resume.as_object_mut() {
        drop(map.insert("stub".to_owned(), json!({"screen": {}})));
    }
    let logs = logged(&sandbox, &resume, QUICK).await.unwrap();
    assert!(logs.contains("line masked again"));
    assert_eq!(
        table(&sandbox, "SELECT state || '|' || text AS item FROM lines").unwrap(),
        ["approved|token [x] here"]
    );
    assert!(!sandbox.read("decide-body.txt").unwrap().contains(SECRET));
}

#[tokio::test]
async fn stored_rules_reach_a_batch_waiting_to_be_sent() {
    let sandbox = Sandbox::new("repack-rules").unwrap();
    let text = format!("token {SECRET} here");
    let rows = [
        (KEY, "u1", "c1", text.as_str()),
        (SECOND, "u1", "c2", text.as_str()),
    ];
    seed(&sandbox, &rows).unwrap();
    let held = json!({
        "limits": {"batch": ONE, "retry_after_ms": BACKOFF},
        "stub": {"decide": {"mode": "fail"}},
    });
    start(&sandbox, &held, QUICK).await.unwrap();
    assert_eq!(table(&sandbox, BATCHES).unwrap(), ["1|ready|1|http:500"]);
    assert!(sandbox.read("decide-body.txt").unwrap().contains(SECRET));
    sandbox.seed(LEARNED).unwrap();
    let resume = json!({"limits": {"batch": ONE}, "stub": {"decide": {"p": "0.1"}}});
    start(&sandbox, &resume, QUICK).await.unwrap();
    assert_eq!(calls(&sandbox, "decide").unwrap(), 3);
    assert_eq!(
        table(&sandbox, BATCHES).unwrap(),
        ["1|sent|0|", "2|sent|0|"]
    );
    assert!(!sandbox.read("decide-body.txt").unwrap().contains(SECRET));
    assert!(!table(&sandbox, PAYLOAD).unwrap().concat().contains(SECRET));
}
