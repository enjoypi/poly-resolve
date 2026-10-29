use serde_json::{Value, json};

use super::rig::{
    KEY, LATER, PLACEMENTS, QUICK, SECOND, TOPICS, decided, one, questions_of, seed, seed_from,
    seed_stamped, start, table, two_chats,
};
use crate::support::{Fallible, Sandbox};

const FIRST_TOPIC: &str = "t1-0";
const QUIET: &str = "0.1";
const TWO_HOURS: u64 = 7_200_000;

#[tokio::test]
async fn another_conversation_can_join_an_open_topic() {
    let patch = json!({"decide": {"env": {"FAKE_P": QUIET, "FAKE_TOPICS": FIRST_TOPIC}}});
    let sandbox = second_run("topic-join", &patch).await.unwrap();
    let body = decided(&sandbox).unwrap();
    assert_eq!(questions_of(&body), ["urgent_1", "topic_1"]);
    assert_eq!(
        body.pointer("/questions/1/choices"),
        Some(&json!([
            {"description": "server d", "value": FIRST_TOPIC},
            {"description": "new topic", "value": "new"},
        ]))
    );
    assert_eq!(
        table(&sandbox, PLACEMENTS).unwrap(),
        [
            format!("{KEY}|{FIRST_TOPIC}|0.1"),
            format!("{SECOND}|{FIRST_TOPIC}|0.1"),
        ]
    );
    assert_eq!(table(&sandbox, TOPICS).unwrap().len(), 1);
}

#[tokio::test]
async fn follow_up_in_the_same_conversation_joins_the_open_topic() {
    let sandbox = Sandbox::new("topic-attach").unwrap();
    seed(&sandbox, &one()).unwrap();
    let quiet = json!({"decide": {"env": {"FAKE_P": QUIET}}});
    start(&sandbox, &quiet, QUICK).await.unwrap();
    seed_from(&sandbox, &[(SECOND, "u1", "c1", "still down")], LATER).unwrap();
    start(&sandbox, &quiet, QUICK).await.unwrap();
    assert_eq!(questions_of(&decided(&sandbox).unwrap()), ["urgent_1"]);
    assert_eq!(
        table(&sandbox, PLACEMENTS).unwrap(),
        [
            format!("{KEY}|{FIRST_TOPIC}|0.1"),
            format!("{SECOND}|{FIRST_TOPIC}|0.1"),
        ]
    );
}

#[tokio::test]
async fn long_pauses_split_a_conversation() {
    let sandbox = Sandbox::new("topic-pause").unwrap();
    let rows = [
        ((KEY, "u1", "c1", "server down"), 1_783_483_235_000),
        (
            (SECOND, "u1", "c1", "still down"),
            1_783_483_235_000 + TWO_HOURS,
        ),
    ];
    seed_stamped(&sandbox, &rows).unwrap();
    start(&sandbox, &json!({}), QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, PLACEMENTS).unwrap(),
        [format!("{KEY}|t1-0|0.95"), format!("{SECOND}|t1-1|0.95")]
    );
}

#[tokio::test]
async fn low_confidence_starts_a_new_topic() {
    let patch = json!({"decide": {"env": {
        "FAKE_CONFIDENCE": "0.2",
        "FAKE_P": QUIET,
        "FAKE_TOPICS": FIRST_TOPIC,
    }}});
    let sandbox = second_run("topic-weak", &patch).await.unwrap();
    assert_eq!(
        table(&sandbox, PLACEMENTS).unwrap(),
        [
            format!("{KEY}|{FIRST_TOPIC}|0.1"),
            format!("{SECOND}|t2-0|0.1")
        ]
    );
}

#[tokio::test]
async fn new_topic_when_the_choice_is_new() {
    let patch = json!({"decide": {"env": {"FAKE_P": QUIET}}});
    let sandbox = second_run("topic-new", &patch).await.unwrap();
    assert_eq!(
        table(&sandbox, PLACEMENTS).unwrap(),
        [
            format!("{KEY}|{FIRST_TOPIC}|0.1"),
            format!("{SECOND}|t2-0|0.1")
        ]
    );
    assert_eq!(table(&sandbox, TOPICS).unwrap().len(), 2);
}

async fn second_run(name: &str, patch: &Value) -> Fallible<Sandbox> {
    let sandbox = Sandbox::new(name)?;
    seed(&sandbox, &one())?;
    let quiet = json!({"decide": {"env": {"FAKE_P": QUIET}}});
    start(&sandbox, &quiet, QUICK).await?;
    seed_from(&sandbox, &[(SECOND, "u1", "c2", "still down")], LATER)?;
    start(&sandbox, patch, QUICK).await?;
    Ok(sandbox)
}

#[tokio::test]
async fn segments_of_one_batch_can_merge() {
    let sandbox = Sandbox::new("topic-merge").unwrap();
    seed(&sandbox, &two_chats()).unwrap();
    let patch = json!({"decide": {"env": {"FAKE_P": QUIET, "FAKE_TOPICS": "x S1"}}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        questions_of(&decided(&sandbox).unwrap()),
        ["urgent_1", "urgent_2", "topic_2"]
    );
    assert_eq!(
        table(&sandbox, PLACEMENTS).unwrap(),
        [
            format!("{KEY}|{FIRST_TOPIC}|0.1"),
            format!("{SECOND}|{FIRST_TOPIC}|0.1"),
        ]
    );
    assert_eq!(table(&sandbox, TOPICS).unwrap(), ["t1-0|server d|"]);
}
