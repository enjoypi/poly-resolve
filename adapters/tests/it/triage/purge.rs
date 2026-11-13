use serde_json::json;

use super::rig::{QUICK, SHORT, calls, one, seed, start, table};
use crate::support::Sandbox;

const AGED: &str =
    "UPDATE topics SET state = 'archived', last_at_ms = 1; UPDATE messages SET occurred_at_ms = 1;";
const SOON: u64 = 1;
const TEXTS: &str = "SELECT m.text || '|' || m.raw || '|' || (m.purged_at_ms > 0) || '|' || l.text AS item FROM messages m JOIN lines l ON l.key = m.key ORDER BY m.key";

#[tokio::test]
async fn archived_topics_lose_their_text_and_session() {
    let sandbox = Sandbox::new("purge-archived").unwrap();
    seed(&sandbox, &one()).unwrap();
    start(&sandbox, &json!({}), SHORT).await.unwrap();
    sandbox.seed(AGED).unwrap();
    let patch = json!({"topics": {"retain": {"after_ms": SOON}}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(table(&sandbox, TEXTS).unwrap(), ["||1|"]);
    assert_eq!(sandbox.read("purge-args.txt").unwrap(), "purge T1\n");
    let topic = "SELECT thread_at_ms || '|' || (purged_at_ms > 0) AS item FROM topics";
    assert_eq!(table(&sandbox, topic).unwrap(), ["0|1"]);
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(calls(&sandbox, "purge").unwrap(), 1);
    let batch = "SELECT payload || '|' || (SELECT COUNT(*) FROM answers) AS item FROM batches";
    assert_eq!(table(&sandbox, batch).unwrap(), ["|0"]);
}

#[tokio::test]
async fn failed_session_purge_is_logged_and_not_retried() {
    let sandbox = Sandbox::new("purge-failed").unwrap();
    seed(&sandbox, &one()).unwrap();
    start(&sandbox, &json!({}), SHORT).await.unwrap();
    sandbox.seed(AGED).unwrap();
    let patch = json!({
        "session": {"purge": {"env": {"FAKE_PURGE": "fail"}}},
        "topics": {"retain": {"after_ms": SOON}},
    });
    start(&sandbox, &patch, QUICK).await.unwrap();
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(calls(&sandbox, "purge").unwrap(), 1);
}

#[tokio::test]
async fn interrupted_session_purge_is_retried() {
    let sandbox = Sandbox::new("purge-stop").unwrap();
    seed(&sandbox, &one()).unwrap();
    start(&sandbox, &json!({}), SHORT).await.unwrap();
    sandbox.seed(AGED).unwrap();
    let patch = json!({
        "session": {"purge": {"env": {"FAKE_PURGE": "slow"}}},
        "topics": {"retain": {"after_ms": SOON}},
    });
    start(&sandbox, &patch, QUICK).await.unwrap();
    let topic = "SELECT CAST(purged_at_ms AS TEXT) AS item FROM topics";
    assert_eq!(table(&sandbox, topic).unwrap(), ["0"]);
}

#[tokio::test]
async fn open_topics_keep_their_text() {
    let sandbox = Sandbox::new("purge-open").unwrap();
    seed(&sandbox, &one()).unwrap();
    start(&sandbox, &json!({}), SHORT).await.unwrap();
    sandbox
        .seed("UPDATE messages SET occurred_at_ms = 1;")
        .unwrap();
    let patch = json!({"topics": {"retain": {"after_ms": SOON}}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, TEXTS).unwrap(),
        ["server down|{}|0|server down"]
    );
    assert_eq!(calls(&sandbox, "purge").unwrap(), 0);
}

#[tokio::test]
async fn reopened_topics_note_the_purged_messages() {
    let sandbox = Sandbox::new("purge-reopen").unwrap();
    seed(&sandbox, &one()).unwrap();
    start(&sandbox, &json!({}), SHORT).await.unwrap();
    sandbox.seed(AGED).unwrap();
    let patch = json!({"topics": {"retain": {"after_ms": SOON}}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    sandbox
        .seed("UPDATE topics SET state = 'open'; INSERT INTO alerts (key, topic_id, state, attempts, analysis, reason, created_at_ms, updated_at_ms) VALUES ('T1@9', 'T1', 'waiting', 0, NULL, '', 9, 0);")
        .unwrap();
    start(&sandbox, &json!({}), SHORT).await.unwrap();
    assert_eq!(sandbox.read("codex-args.txt").unwrap(), "codex -\n");
    let prompt = sandbox.read("codex-stdin.txt").unwrap();
    assert!(prompt.contains("1 held: purged"), "{prompt}");
}
