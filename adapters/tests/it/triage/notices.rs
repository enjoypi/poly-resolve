use std::fs;

use serde_json::json;

use super::rig::{
    ALERTS, LATER, QUICK, SECOND, SHORT, calls, logged, one, seed, seed_from, start, table,
    two_chats,
};
use crate::support::{Fallible, Sandbox};

const COOLDOWN: u64 = 3_600_000;
const ONE: u32 = 1;
const REOPEN: &str =
    "UPDATE alerts SET state = 'analyzed', attempts = 0; DELETE FROM calls WHERE kind = 'notify';";
const STATES: &str =
    "SELECT key || '|' || state || '|' || attempts AS item FROM alerts ORDER BY key";

async fn analysed(name: &str) -> Fallible<Sandbox> {
    let sandbox = Sandbox::new(name)?;
    seed(&sandbox, &two_chats())?;
    start(&sandbox, &json!({}), SHORT).await?;
    sandbox.seed(REOPEN)?;
    Ok(sandbox)
}

#[tokio::test]
async fn backlog_is_summarized_once_after_a_restart() {
    let sandbox = analysed("notices-recap").await.unwrap();
    let before = calls(&sandbox, "send").unwrap();
    start(&sandbox, &json!({}), QUICK).await.unwrap();
    assert_eq!(calls(&sandbox, "send").unwrap(), before + 1);
    assert_eq!(
        sandbox.read("send-stdin.txt").unwrap(),
        "2 topics:\nserver d/open/0.95\nstill do/open/0.95"
    );
    assert!(
        sandbox
            .read("send-args.txt")
            .unwrap()
            .contains("--idempotency-key=summary@")
    );
    assert_eq!(
        table(&sandbox, STATES).unwrap(),
        ["t1-0@1|sent|0", "t1-1@1|sent|0"]
    );
}

#[tokio::test]
async fn cooldown_holds_the_next_notice_of_a_topic() {
    let sandbox = Sandbox::new("notices-cooldown").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"limits": {"notice_cooldown_ms": COOLDOWN}});
    start(&sandbox, &patch, SHORT).await.unwrap();
    seed_from(&sandbox, &[(SECOND, "u1", "c1", "still down")], LATER).unwrap();
    start(&sandbox, &patch, SHORT).await.unwrap();
    assert_eq!(
        table(&sandbox, ALERTS).unwrap(),
        ["T1@2|analyzed|0|ANALYSIS|", "t1-0@1|sent|0|ANALYSIS|"]
    );
    assert_eq!(calls(&sandbox, "send").unwrap(), 1);
}

#[tokio::test]
async fn failed_summary_falls_back_to_single_notices() {
    let sandbox = analysed("notices-recap-fail").await.unwrap();
    let patch = json!({"notify": {"env": {"FAKE_SEND": "fail"}}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, STATES).unwrap(),
        ["t1-0@1|failed|2", "t1-1@1|failed|2"]
    );
}

#[tokio::test]
async fn interrupted_summary_is_released() {
    let sandbox = analysed("notices-recap-stop").await.unwrap();
    let patch = json!({"notify": {"env": {"FAKE_SEND": "slow"}}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, STATES).unwrap(),
        ["t1-0@1|analyzed|0", "t1-1@1|analyzed|0"]
    );
}

#[tokio::test]
async fn older_conclusions_give_way_to_newer_ones() {
    let sandbox = Sandbox::new("notices-supersede").unwrap();
    seed(&sandbox, &one()).unwrap();
    start(&sandbox, &json!({}), SHORT).await.unwrap();
    sandbox
        .seed("INSERT INTO alerts (key, topic_id, state, attempts, analysis, reason, created_at_ms, updated_at_ms) VALUES ('T1@7', 'T1', 'analyzed', 0, 'older', '', 7, 0), ('T1@8', 'T1', 'analyzed', 0, 'newer', '', 8, 0);")
        .unwrap();
    let logs = logged(&sandbox, &json!({}), QUICK).await.unwrap();
    assert!(logs.contains("older notices merged"));
    assert_eq!(
        table(&sandbox, STATES).unwrap(),
        ["T1@7|merged|0", "T1@8|sent|0", "t1-0@1|sent|0"]
    );
    assert!(sandbox.read("send-stdin.txt").unwrap().ends_with("|newer"));
}

#[tokio::test]
async fn pending_newer_alerts_keep_the_older_round_quiet() {
    let sandbox = Sandbox::new("notices-quiet").unwrap();
    seed(&sandbox, &one()).unwrap();
    fs::remove_file(sandbox.prompts().unwrap().join("session.md")).unwrap();
    start(&sandbox, &json!({}), QUICK).await.unwrap();
    sandbox
        .seed("INSERT INTO alerts (key, topic_id, state, attempts, analysis, reason, created_at_ms, updated_at_ms) VALUES ('t1-0@9', 't1-0', 'waiting', 0, NULL, '', 9999999999999999, 0);")
        .unwrap();
    fs::write(
        sandbox.prompts().unwrap().join("session.md"),
        "ctx:{messages}",
    )
    .unwrap();
    let patch = json!({"limits": {"parallel": ONE}});
    start(&sandbox, &patch, SHORT).await.unwrap();
    assert_eq!(
        table(&sandbox, ALERTS).unwrap(),
        ["t1-0@1|quiet|0|ANALYSIS|", "t1-0@9|sent|0|ANALYSIS|"]
    );
}

#[tokio::test]
async fn summary_waits_when_claims_cannot_be_stored() {
    let sandbox = analysed("notices-recap-claim").await.unwrap();
    let before = calls(&sandbox, "send").unwrap();
    sandbox
        .seed("CREATE TRIGGER refuse BEFORE UPDATE OF attempts ON alerts BEGIN SELECT RAISE(ABORT, 'refused'); END;")
        .unwrap();
    start(&sandbox, &json!({}), QUICK).await.unwrap();
    assert_eq!(calls(&sandbox, "send").unwrap(), before);
    assert_eq!(
        table(&sandbox, STATES).unwrap(),
        ["t1-0@1|analyzed|0", "t1-1@1|analyzed|0"]
    );
}
