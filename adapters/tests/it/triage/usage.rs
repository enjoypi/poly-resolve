use serde_json::json;

use super::rig::{ALERTS, LATER, SECOND, SHORT, calls, one, seed, seed_from, start, table};
use crate::support::Sandbox;

const SPENT: &str = "SELECT kind || '|' || state || '|' || input_tokens || '|' || output_tokens || '|' || length(version) AS item FROM calls ORDER BY kind, subject, attempt";

#[tokio::test]
async fn calls_record_outcome_tokens_and_prompt_version() {
    let sandbox = Sandbox::new("usage-calls").unwrap();
    seed(&sandbox, &one()).unwrap();
    start(&sandbox, &json!({}), SHORT).await.unwrap();
    assert_eq!(
        table(&sandbox, SPENT).unwrap(),
        [
            "analysis|success|20|5|12",
            "decision|success|11|3|12",
            "notify|success|0|0|0"
        ]
    );
    let version = "SELECT length(version) || '|' || (version = (SELECT version FROM calls WHERE kind = 'analysis')) AS item FROM topics";
    assert_eq!(table(&sandbox, version).unwrap(), ["12|1"]);
    let batch = "SELECT CAST(length(version) AS TEXT) AS item FROM batches";
    assert_eq!(table(&sandbox, batch).unwrap(), ["12"]);
}

#[tokio::test]
async fn failed_calls_are_counted_as_failures() {
    let sandbox = Sandbox::new("usage-failed").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({
        "codex": {"env": {"FAKE_CODEX": "fail"}},
        "stub": {"decide": {"mode": "fail"}},
    });
    start(&sandbox, &patch, SHORT).await.unwrap();
    let states = "SELECT DISTINCT kind || '|' || state AS item FROM calls ORDER BY item";
    assert_eq!(table(&sandbox, states).unwrap(), ["decision|failed"]);
    sandbox
        .seed("UPDATE batches SET state = 'ready', revivals = 1, updated_at_ms = 0")
        .unwrap();
    start(
        &sandbox,
        &json!({"codex": {"env": {"FAKE_CODEX": "fail"}}, "stub": {"decide": {}}}),
        SHORT,
    )
    .await
    .unwrap();
    assert_eq!(
        table(&sandbox, states).unwrap(),
        ["analysis|failed", "decision|failed", "decision|success"]
    );
}

#[tokio::test]
async fn unchanged_topics_are_not_analyzed_again() {
    let sandbox = Sandbox::new("usage-unchanged").unwrap();
    seed(&sandbox, &one()).unwrap();
    start(&sandbox, &json!({}), SHORT).await.unwrap();
    sandbox
        .seed("INSERT INTO alerts (key, topic_id, state, attempts, analysis, reason, created_at_ms, updated_at_ms) VALUES ('T1@9', 'T1', 'waiting', 0, NULL, '', 1, 0);")
        .unwrap();
    start(&sandbox, &json!({}), SHORT).await.unwrap();
    assert_eq!(calls(&sandbox, "codex").unwrap(), 1);
    assert_eq!(
        table(&sandbox, ALERTS).unwrap(),
        ["T1@9|quiet|0||", "t1-0@1|sent|0|ANALYSIS|"]
    );
    seed_from(&sandbox, &[(SECOND, "u1", "c1", "still down")], LATER).unwrap();
    start(&sandbox, &json!({}), SHORT).await.unwrap();
    assert_eq!(calls(&sandbox, "codex").unwrap(), 2);
}
