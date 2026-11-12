use serde_json::json;

use super::rig::{
    ALERTS, KEY, LATER, QUICK, SECOND, SHORT, THIRD, TOPICS, calls, decided, logged, now, one,
    questions_of, seed, seed_from, seed_stamped, start, table, two_chats,
};
use crate::support::Sandbox;

const CLOSED: &str = "SELECT CAST(COUNT(*) AS TEXT) AS item FROM topics WHERE closed_at_ms > 0";
const IDLE: u64 = 60_000;
const NONE: f64 = 0.0;
const ONE: u32 = 1;
const SCORES: &str =
    "SELECT id || '|' || round(score, 2) || '|' || reach AS item FROM topics ORDER BY id";
const SOON: u64 = 1;
const STATES: &str = "SELECT id || '|' || state AS item FROM topics ORDER BY id";
const WIDE: f64 = 0.9;

#[tokio::test]
async fn higher_scores_are_analysed_first() {
    let sandbox = Sandbox::new("life-order").unwrap();
    seed(&sandbox, &two_chats()).unwrap();
    let patch = json!({
        "codex": {"env": {"FAKE_CODEX": "slow"}},
        "limits": {"parallel": ONE},
        "stub": {"decide": {"ps": "0.85 0.95"}},
    });
    start(&sandbox, &patch, QUICK).await.unwrap();
    let prompt = sandbox.read("codex-stdin.txt").unwrap();
    assert!(prompt.contains("still down") && !prompt.contains("server down"));
}

#[tokio::test]
async fn idle_topics_are_archived_until_a_clue_brings_them_back() {
    let sandbox = Sandbox::new("life-archive").unwrap();
    seed(&sandbox, &[(KEY, "u1", "c1", "OPS-1 down")]).unwrap();
    let patch = json!({
        "stub": {"decide": {"p": "0.1"}},
        "topics": {"archive_after_ms": IDLE},
    });
    let logs = logged(&sandbox, &patch, QUICK).await.unwrap();
    assert!(logs.contains("idle topics archived"));
    assert_eq!(table(&sandbox, STATES).unwrap(), ["t1-0|archived"]);
    seed_from(&sandbox, &[(SECOND, "u1", "c2", "something else")], LATER).unwrap();
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(questions_of(&decided(&sandbox).unwrap()), ["urgent_1"]);
    assert_eq!(
        table(&sandbox, STATES).unwrap(),
        ["t1-0|archived", "t2-0|archived"]
    );
    let fresh = now().unwrap();
    seed_stamped(&sandbox, &[((THIRD, "u1", "c3", "OPS-1 again"), fresh)]).unwrap();
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, STATES).unwrap(),
        ["t1-0|open", "t2-0|archived"]
    );
}

#[tokio::test]
async fn late_messages_are_marked_in_the_notice() {
    let sandbox = Sandbox::new("life-late").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"notice": {"late_after_ms": SOON}});
    start(&sandbox, &patch, SHORT).await.unwrap();
    assert_eq!(
        sandbox.read("send-stdin.txt").unwrap(),
        "[0.95] open server d T1 late 2026-07-08 04:00:35 UTC||ANALYSIS"
    );
}

#[tokio::test]
async fn reach_lifts_a_quiet_topic_over_the_threshold() {
    let sandbox = Sandbox::new("life-reach").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({
        "stub": {"decide": {"p": "0.1", "topics": "t1-0"}},
        "topics": {"reach": {"cap": WIDE, "conversations": NONE, "people": WIDE, "sources": NONE}},
    });
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, SCORES).unwrap(),
        [r#"t1-0|0.1|{"conversations":1,"people":1,"sources":1}"#]
    );
    assert_eq!(calls(&sandbox, "codex").unwrap(), 0);
    seed_from(&sandbox, &[(SECOND, "u2", "c2", "same here")], LATER).unwrap();
    start(&sandbox, &patch, SHORT).await.unwrap();
    assert_eq!(
        table(&sandbox, SCORES).unwrap(),
        [r#"T1|0.91|{"conversations":2,"people":2,"sources":1}"#]
    );
    assert_eq!(
        table(&sandbox, ALERTS).unwrap(),
        ["t1-0@2|sent|0|ANALYSIS|"]
    );
}

#[tokio::test]
async fn resolved_topics_reopen_on_new_members() {
    let sandbox = Sandbox::new("life-reopen").unwrap();
    seed(&sandbox, &one()).unwrap();
    let resolved = json!({"codex": {"env": {"FAKE_CODEX": "resolved"}}});
    start(&sandbox, &resolved, SHORT).await.unwrap();
    assert_eq!(table(&sandbox, STATES).unwrap(), ["T1|resolved"]);
    assert_eq!(table(&sandbox, CLOSED).unwrap(), ["1"]);
    assert_eq!(
        sandbox.read("send-stdin.txt").unwrap(),
        "[0.95] done server d T1||ANALYSIS"
    );
    seed_from(&sandbox, &[(SECOND, "u1", "c1", "still down")], LATER).unwrap();
    let quiet = json!({"stub": {"decide": {"p": "0.1"}}});
    start(&sandbox, &quiet, SHORT).await.unwrap();
    assert_eq!(table(&sandbox, STATES).unwrap(), ["T1|open"]);
    assert_eq!(table(&sandbox, CLOSED).unwrap(), ["0"]);
    assert_eq!(
        table(&sandbox, ALERTS).unwrap(),
        ["T1@2|sent|0|ANALYSIS|", "t1-0@1|sent|0|ANALYSIS|"]
    );
}

#[tokio::test]
async fn titles_and_changes_reach_the_topic_and_the_notice() {
    let sandbox = Sandbox::new("life-title").unwrap();
    seed(&sandbox, &one()).unwrap();
    let titled = json!({"codex": {"env": {"FAKE_CODEX": "titled"}}});
    start(&sandbox, &titled, SHORT).await.unwrap();
    assert_eq!(table(&sandbox, TOPICS).unwrap(), ["T1|TITLE|T1"]);
    assert_eq!(
        sandbox.read("send-stdin.txt").unwrap(),
        "[0.95] open TITLE T1|moved|ANALYSIS"
    );
}

#[tokio::test]
async fn unchanged_conclusions_stay_quiet() {
    let sandbox = Sandbox::new("life-quiet").unwrap();
    seed(&sandbox, &one()).unwrap();
    start(&sandbox, &json!({}), SHORT).await.unwrap();
    seed_from(&sandbox, &[(SECOND, "u1", "c1", "still down")], LATER).unwrap();
    let same = json!({"codex": {"env": {"FAKE_CODEX": "same"}}});
    start(&sandbox, &same, SHORT).await.unwrap();
    assert_eq!(
        table(&sandbox, ALERTS).unwrap(),
        ["T1@2|quiet|0|ANALYSIS|", "t1-0@1|sent|0|ANALYSIS|"]
    );
    assert_eq!(calls(&sandbox, "send").unwrap(), 1);
}
