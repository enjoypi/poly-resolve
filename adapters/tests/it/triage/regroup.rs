use std::fs;

use serde_json::json;

use super::rig::{
    KEY, LATER, PLACEMENTS, QUICK, SECOND, SHORT, THIRD, TOPICS, decided, logged, questions_of,
    seed, seed_from, start, table,
};
use crate::support::{Fallible, Sandbox};

const ALIASES: &str = "SELECT old || '|' || new AS item FROM topic_aliases ORDER BY old";
const CHANGES: &str = "SELECT kind || '|' || from_topic || '|' || to_topic || '|' || state || '|' || reason AS item FROM changes ORDER BY id";
const FOURTH: &str = "dingtalk:acct:m4";
const RELATIONS: &str = "SELECT a || '|' || b || '|' || kind || '|' || count AS item FROM relations ORDER BY a, b, kind";
const STATES: &str = "SELECT key || '|' || state AS item FROM alerts ORDER BY key";
const VIA: &str = "SELECT key || '|' || topic_id || '|' || via AS item FROM members ORDER BY key";

#[tokio::test]
async fn blocked_and_orphaned_changes_wait_or_drop() {
    let sandbox = Sandbox::new("regroup-blocked").unwrap();
    seed(
        &sandbox,
        &[
            (KEY, "u1", "c1", "server down"),
            (SECOND, "u2", "c2", "disk full"),
        ],
    )
    .unwrap();
    fs::remove_file(sandbox.prompts().unwrap().join("session.md")).unwrap();
    start(&sandbox, &json!({}), QUICK).await.unwrap();
    sandbox
        .seed("UPDATE alerts SET attempts = 1 WHERE topic_id = 't1-0'; INSERT INTO changes (kind, from_topic, to_topic, keys, reason, state, at_ms) VALUES ('merge', 't1-1', 't1-0', '[]', 'vote', 'pending', 1), ('merge', 't9-9', 't1-0', '[]', 'vote', 'pending', 1);")
        .unwrap();
    start(&sandbox, &json!({}), QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, CHANGES).unwrap(),
        [
            "merge|t1-1|t1-0|pending|vote",
            "merge|t9-9|t1-0|dropped|vote"
        ]
    );
    sandbox.seed("UPDATE alerts SET attempts = 0").unwrap();
    start(&sandbox, &json!({}), QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, CHANGES).unwrap(),
        [
            "merge|t1-1|t1-0|applied|vote",
            "merge|t9-9|t1-0|dropped|vote"
        ]
    );
    assert_eq!(
        table(&sandbox, STATES).unwrap(),
        ["t1-0@1|pending", "t1-1@1|merged"]
    );
    assert_eq!(table(&sandbox, TOPICS).unwrap().len(), 1);
}

#[tokio::test]
async fn doubted_choices_split_into_their_own_topic() {
    let sandbox = Sandbox::new("regroup-split").unwrap();
    seed(&sandbox, &[(KEY, "u1", "c1", "server down")]).unwrap();
    let quiet = json!({"stub": {"decide": {"p": "0.1"}}});
    start(&sandbox, &quiet, QUICK).await.unwrap();
    seed_from(&sandbox, &[(SECOND, "u2", "c2", "lunch plan")], LATER).unwrap();
    let joined = json!({"stub": {"decide": {"p": "0.1", "topics": "t1-0"}}});
    start(&sandbox, &joined, QUICK).await.unwrap();
    let doubt = json!({"stub": {"decide": {"p": "0.1", "review": "0.1"}}});
    seed_from(&sandbox, &[(THIRD, "u3", "c3", "third thing")], 2 * LATER).unwrap();
    start(&sandbox, &doubt, QUICK).await.unwrap();
    assert_eq!(
        questions_of(&decided(&sandbox).unwrap()),
        ["urgent_1", "review_1", "topic_1"]
    );
    assert_eq!(
        table(&sandbox, RELATIONS).unwrap(),
        [format!("{SECOND}|t1-0|apart|1")]
    );
    seed_from(&sandbox, &[(FOURTH, "u4", "c4", "fourth thing")], 3 * LATER).unwrap();
    start(&sandbox, &doubt, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, CHANGES).unwrap(),
        ["split|t1-0|s1|applied|apart"]
    );
    assert_eq!(
        table(&sandbox, VIA).unwrap(),
        [
            format!("{KEY}|t1-0|new"),
            format!("{SECOND}|s1|split"),
            format!("{THIRD}|t3-0|new"),
            format!("{FOURTH}|t4-0|new"),
        ]
    );
    assert_eq!(table(&sandbox, RELATIONS).unwrap(), [""; 0]);
}

#[tokio::test]
async fn kept_choices_are_not_asked_again() {
    let sandbox = Sandbox::new("regroup-kept").unwrap();
    seed(&sandbox, &[(KEY, "u1", "c1", "server down")]).unwrap();
    let quiet = json!({"stub": {"decide": {"p": "0.1"}}});
    start(&sandbox, &quiet, QUICK).await.unwrap();
    seed_from(&sandbox, &[(SECOND, "u2", "c2", "still down")], LATER).unwrap();
    let joined = json!({"stub": {"decide": {"p": "0.1", "topics": "t1-0"}}});
    start(&sandbox, &joined, QUICK).await.unwrap();
    seed_from(&sandbox, &[(THIRD, "u3", "c3", "third thing")], 2 * LATER).unwrap();
    start(&sandbox, &quiet, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, RELATIONS).unwrap(),
        [format!("{SECOND}|t1-0|kept|1")]
    );
    seed_from(&sandbox, &[(FOURTH, "u4", "c4", "fourth thing")], 3 * LATER).unwrap();
    start(&sandbox, &quiet, QUICK).await.unwrap();
    assert_eq!(
        questions_of(&decided(&sandbox).unwrap()),
        ["urgent_1", "topic_1"]
    );
}

#[tokio::test]
async fn minor_overlaps_move_only_the_shared_lines() {
    let sandbox = Sandbox::new("regroup-move").unwrap();
    seed(
        &sandbox,
        &[
            (KEY, "u1", "c1", "OPS-1 down"),
            (SECOND, "u2", "c2", "hello"),
            (THIRD, "u2", "c2", "see OPS-1"),
            (FOURTH, "u2", "c2", "bye"),
        ],
    )
    .unwrap();
    let quiet = json!({"stub": {"decide": {"p": "0.1"}}});
    start(&sandbox, &quiet, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, CHANGES).unwrap(),
        ["move|t1-1|t1-0|applied|clue"]
    );
    assert_eq!(
        table(&sandbox, VIA).unwrap(),
        [
            format!("{KEY}|t1-0|new"),
            format!("{SECOND}|t1-1|new"),
            format!("{THIRD}|t1-0|move"),
            format!("{FOURTH}|t1-1|new"),
        ]
    );
}

#[tokio::test]
async fn repeated_votes_merge_related_topics_and_tell_the_owner() {
    let sandbox = voted("regroup-vote", "0.95").await.unwrap();
    assert_eq!(
        table(&sandbox, CHANGES).unwrap(),
        ["merge|t2-0|T1|applied|vote"]
    );
    assert_eq!(
        table(&sandbox, PLACEMENTS).unwrap().len(),
        table(&sandbox, VIA).unwrap().len()
    );
    assert_eq!(table(&sandbox, ALIASES).unwrap(), ["t1-0|T1", "t2-0|T1"]);
    assert_eq!(table(&sandbox, RELATIONS).unwrap(), [""; 0]);
    let notice = sandbox.read("send-stdin.txt").unwrap();
    assert!(notice.contains(" T1 merged|"), "{notice}");
    assert_eq!(
        table(&sandbox, STATES).unwrap(),
        ["T1@c1|sent", "t1-0@1|sent"]
    );
}

#[tokio::test]
async fn shared_clues_merge_topics_of_one_batch() {
    let sandbox = Sandbox::new("regroup-clue").unwrap();
    seed(
        &sandbox,
        &[
            (KEY, "u1", "c1", "OPS-1 down"),
            (SECOND, "u2", "c2", "OPS-1 again"),
        ],
    )
    .unwrap();
    let quiet = json!({"stub": {"decide": {"p": "0.1"}}});
    let logs = logged(&sandbox, &quiet, QUICK).await.unwrap();
    assert!(logs.contains("topic changes proposed") && logs.contains("topic change settled"));
    assert_eq!(
        table(&sandbox, CHANGES).unwrap(),
        ["merge|t1-1|t1-0|applied|clue"]
    );
    assert_eq!(
        table(&sandbox, PLACEMENTS).unwrap(),
        [format!("{KEY}|t1-0|0.1"), format!("{SECOND}|t1-0|0.1")]
    );
    assert_eq!(table(&sandbox, ALIASES).unwrap(), ["t1-1|t1-0"]);
    assert_eq!(table(&sandbox, TOPICS).unwrap(), ["t1-0|OPS-1 do|"]);
}

#[tokio::test]
async fn unlike_votes_stop_the_question() {
    let sandbox = voted("regroup-unlike", "0.1").await.unwrap();
    assert_eq!(
        table(&sandbox, RELATIONS).unwrap(),
        ["t2-0|T1|maybe|1", "t2-0|T1|unlike|1"]
    );
    let more = json!({"stub": {"decide": {"p": "0.1", "review": "0.1"}}});
    seed_from(&sandbox, &[(FOURTH, "u4", "c4", "fourth thing")], 3 * LATER).unwrap();
    start(&sandbox, &more, QUICK).await.unwrap();
    assert_eq!(table(&sandbox, RELATIONS).unwrap(), [""; 0]);
    assert_eq!(table(&sandbox, CHANGES).unwrap(), [""; 0]);
}

async fn voted(name: &str, review: &str) -> Fallible<Sandbox> {
    let sandbox = Sandbox::new(name)?;
    seed(&sandbox, &[(KEY, "u1", "c1", "server down")])?;
    start(&sandbox, &json!({"stub": {"decide": {}}}), SHORT).await?;
    seed_from(&sandbox, &[(SECOND, "u1", "c2", "maybe related")], LATER)?;
    let unsure = json!({"stub": {"decide": {"confidence": "0.2", "p": "0.1", "topics": "T1"}}});
    start(&sandbox, &unsure, QUICK).await?;
    seed_from(&sandbox, &[(THIRD, "u3", "c3", "third thing")], 2 * LATER)?;
    let asked = json!({"stub": {"decide": {"p": "0.1", "review": review}}});
    start(&sandbox, &asked, SHORT).await?;
    Ok(sandbox)
}
