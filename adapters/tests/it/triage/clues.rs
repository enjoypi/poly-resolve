use adapters::store::Archive;
use serde_json::{Value, json};

use super::rig::{
    KEY, LATER, PLACEMENTS, QUICK, SECOND, THIRD, decided, logged, questions_of, seed, seed_from,
    start, table,
};
use crate::support::{BUSY, NOON, Sandbox};

const APART: u64 = 7_200_000;
const CHANGES: &str = "SELECT kind || '|' || from_topic || '|' || to_topic || '|' || reason AS item FROM changes ORDER BY id";
const CLUES: &str = "SELECT kind || '|' || value || '|' || key AS item FROM clues WHERE kind <> 'seen' ORDER BY kind, value, key";
const ONE: u32 = 1;
const RELATIONS: &str =
    "SELECT a || '|' || b || '|' || kind AS item FROM relations ORDER BY a, b, kind";
const TWO: u32 = 2;
const VIA: &str = "SELECT key || '|' || via AS item FROM members ORDER BY key";

fn capped(limit: u32) -> Value {
    json!({"stub": {"decide": {"p": "0.1"}}, "topics": {"clues": {"max_conversations": limit}}})
}

#[tokio::test]
async fn invalid_clue_patterns_hold_the_round() {
    let sandbox = Sandbox::new("clues-invalid").unwrap();
    seed(&sandbox, &[(KEY, "u1", "c1", "OPS-1 down")]).unwrap();
    let patch = json!({"topics": {"clues": {"patterns": [{"kind": "x", "pattern": "("}]}}});
    let logs = logged(&sandbox, &patch, QUICK).await.unwrap();
    assert!(logs.contains("clues unavailable"));
    assert_eq!(table(&sandbox, CLUES).unwrap(), [""; 0]);
}

#[tokio::test]
async fn mail_replies_join_the_topic_of_their_thread() {
    let sandbox = Sandbox::new("clues-mail").unwrap();
    let archive = Archive::open(&sandbox.database(), BUSY).unwrap();
    for (key, conversation, raw, stamp) in [
        (
            KEY,
            "c1",
            r#"{"message_id":"m1","subject":"Deploy plan"}"#,
            NOON,
        ),
        (
            SECOND,
            "c2",
            r#"{"in_reply_to":"m1","message_id":"m2","subject":"RE: deploy plan"}"#,
            NOON + LATER,
        ),
    ] {
        let envelope = json!({
            "account": "acct",
            "conversation_id": conversation,
            "key": key,
            "occurred_at_ms": stamp,
            "raw": raw,
            "received_at_ms": stamp,
            "sender_id": "u1",
            "sender_name": "name",
            "source": "dingtalk",
            "text": "hello",
        });
        let _: usize = archive.save_event(&envelope.to_string()).unwrap();
    }
    start(&sandbox, &capped(ONE), QUICK).await.unwrap();
    assert_eq!(table(&sandbox, CHANGES).unwrap(), ["merge|t1-1|t1-0|mail"]);
    assert_eq!(
        table(&sandbox, CLUES).unwrap(),
        [
            format!("mail|m1|{KEY}"),
            format!("mail|m1|{SECOND}"),
            format!("mail|m2|{SECOND}"),
            format!("subject|deploy plan|{KEY}"),
            format!("subject|deploy plan|{SECOND}"),
        ]
    );
}

#[tokio::test]
async fn one_conversation_counts_once_toward_the_limit() {
    let sandbox = Sandbox::new("clues-same-conversation").unwrap();
    seed(&sandbox, &[(KEY, "u1", "c1", "OPS-1 down")]).unwrap();
    start(&sandbox, &capped(ONE), QUICK).await.unwrap();
    seed_from(&sandbox, &[(SECOND, "u2", "c1", "OPS-1 again")], APART).unwrap();
    start(&sandbox, &capped(ONE), QUICK).await.unwrap();
    assert_eq!(questions_of(&decided(&sandbox).unwrap()), ["urgent_1"]);
    assert_eq!(
        table(&sandbox, VIA).unwrap(),
        [format!("{KEY}|new"), format!("{SECOND}|clue")]
    );
}

#[tokio::test]
async fn shared_clues_join_topics_across_conversations() {
    let sandbox = Sandbox::new("clues-join").unwrap();
    seed(&sandbox, &[(KEY, "u1", "c1", "OPS-1 down")]).unwrap();
    let quiet = json!({"stub": {"decide": {"p": "0.1"}}});
    start(&sandbox, &quiet, QUICK).await.unwrap();
    seed_from(&sandbox, &[(SECOND, "u2", "c2", "see OPS-1")], LATER).unwrap();
    start(&sandbox, &quiet, QUICK).await.unwrap();
    assert_eq!(questions_of(&decided(&sandbox).unwrap()), ["urgent_1"]);
    assert_eq!(
        table(&sandbox, PLACEMENTS).unwrap(),
        [format!("{KEY}|t1-0|0.1"), format!("{SECOND}|t1-0|0.1")]
    );
    assert_eq!(
        table(&sandbox, VIA).unwrap(),
        [format!("{KEY}|new"), format!("{SECOND}|clue")]
    );
}

#[tokio::test]
async fn weak_clues_pointing_at_two_topics_ask_the_model() {
    let sandbox = Sandbox::new("clues-fanout").unwrap();
    seed(
        &sandbox,
        &[
            (KEY, "u1", "c1", "OPS-1 down"),
            (SECOND, "u2", "c2", "OPS-2 down"),
        ],
    )
    .unwrap();
    start(&sandbox, &capped(TWO), QUICK).await.unwrap();
    seed_from(&sandbox, &[(THIRD, "u3", "c3", "OPS-1 and OPS-2")], LATER).unwrap();
    start(&sandbox, &capped(TWO), QUICK).await.unwrap();
    assert_eq!(
        questions_of(&decided(&sandbox).unwrap()),
        ["urgent_1", "topic_1"]
    );
    assert_eq!(
        table(&sandbox, VIA).unwrap(),
        [
            format!("{KEY}|new"),
            format!("{SECOND}|new"),
            format!("{THIRD}|new")
        ]
    );
}

#[tokio::test]
async fn widespread_clues_stop_joining_topics() {
    let sandbox = Sandbox::new("clues-widespread").unwrap();
    seed(&sandbox, &[(KEY, "u1", "c1", "OPS-1 down")]).unwrap();
    start(&sandbox, &capped(TWO), QUICK).await.unwrap();
    seed_from(&sandbox, &[(SECOND, "u2", "c2", "see OPS-1")], LATER).unwrap();
    start(&sandbox, &capped(TWO), QUICK).await.unwrap();
    seed_from(&sandbox, &[(THIRD, "u3", "c3", "OPS-1 again")], 2 * LATER).unwrap();
    start(&sandbox, &capped(TWO), QUICK).await.unwrap();
    assert_eq!(
        questions_of(&decided(&sandbox).unwrap()),
        ["urgent_1", "topic_1"]
    );
    assert_eq!(
        table(&sandbox, VIA).unwrap(),
        [
            format!("{KEY}|new"),
            format!("{SECOND}|clue"),
            format!("{THIRD}|new")
        ]
    );
    assert_eq!(table(&sandbox, RELATIONS).unwrap(), [""; 0]);
}
