use serde_json::{Error, Value, from_str, json};

use super::{Reply, call, patched};

const FIRST_QUESTION: usize = 1;
const MINUTE: u64 = 60_000;
const NOON: u64 = 1_783_483_235_000;
const ROOMY: u32 = 10_000;
const SECOND_QUESTION: usize = 2;
const WINDOW: u64 = 2 * MINUTE;

fn body_of(reply: &Value) -> Value {
    reply
        .get("body")
        .and_then(Value::as_str)
        .and_then(|text| from_str(text).ok())
        .unwrap_or_default()
}

fn entry(who: [&str; 3], at_ms: u64) -> Value {
    let [key, conversation, text] = who;
    json!({
        "conversation_id": conversation,
        "key": key,
        "occurred_at_ms": at_ms,
        "sender": "alice",
        "text": text,
    })
}

fn input_of(body: &Value) -> Value {
    body.get("input")
        .and_then(Value::as_str)
        .and_then(|text| from_str(text).ok())
        .unwrap_or_default()
}

fn pack(lines: &Value, attach: &Value, topics: &Value) -> Result<Reply, Error> {
    call("pack", &request(lines, attach, topics).to_string())
}

#[test]
fn pack_asks_to_merge_new_segments_into_known_topics() {
    let lines = json!([entry(["k1", "c1", "hello world"], NOON)]);
    let topics = json!([{"label": "old", "topic": "t3-0"}]);
    let reply = pack(&lines, &json!([]), &topics).unwrap().unwrap();
    let body = body_of(&reply);
    assert_eq!(
        body.get("questions"),
        Some(&json!([
            {"instructions": "urgent L1", "name": "urgent_1", "type": "predicate"},
            {
                "choices": [
                    {"description": "old", "value": "t3-0"},
                    {"description": "new topic", "value": "new"},
                ],
                "instructions": "which S1",
                "name": "topic_1",
                "type": "choice",
            },
        ]))
    );
    let layout = reply.get("layout").unwrap();
    let segment = layout.get("segments").and_then(Value::as_array).unwrap();
    assert_eq!(
        segment.first().and_then(|first| first.get("question")),
        Some(&json!(FIRST_QUESTION))
    );
}

#[test]
fn pack_attaches_segments_to_the_open_topic_of_their_conversation() {
    let lines = json!([
        entry(["k1", "c1", "hello"], NOON),
        entry(["k2", "c2", "yo"], NOON),
    ]);
    let attach = json!([{"conversation_id": "c1", "last_ms": NOON - MINUTE, "topic_id": "t9-0"}]);
    let topics = json!([{"label": "open", "topic": "t9-0"}]);
    let reply = pack(&lines, &attach, &topics).unwrap().unwrap();
    let layout = reply.get("layout").unwrap();
    let segments = layout.get("segments").and_then(Value::as_array).unwrap();
    let first = segments.first().unwrap();
    assert_eq!(first.get("attach"), Some(&json!("t9-0")));
    assert_eq!(first.get("question"), Some(&Value::Null));
    assert_eq!(first.get("choices"), Some(&json!([])));
    let second = segments.last().unwrap();
    assert_eq!(second.get("attach"), Some(&Value::Null));
    assert_eq!(second.get("question"), Some(&json!(SECOND_QUESTION)));
    let questions = body_of(&reply);
    assert_eq!(
        questions
            .get("questions")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(SECOND_QUESTION + 1)
    );
}

#[test]
fn pack_ignores_stale_or_foreign_attachments() {
    let lines = json!([entry(["k1", "c1", "hello"], NOON)]);
    let attach = json!([
        {"conversation_id": "c2", "last_ms": NOON, "topic_id": "foreign"},
        {"conversation_id": "c1", "last_ms": NOON - 10 * MINUTE, "topic_id": "stale"},
    ]);
    let reply = pack(&lines, &attach, &json!([])).unwrap().unwrap();
    let layout = reply.get("layout").unwrap();
    let segments = layout.get("segments").and_then(Value::as_array).unwrap();
    assert_eq!(
        segments.first().and_then(|first| first.get("attach")),
        Some(&Value::Null)
    );
}

#[test]
fn pack_labels_masks_and_aliases() {
    let lines = json!([
        entry(["k1", "c1", "token abc1234"], NOON),
        entry(["k2", "c2", "second"], NOON + 1),
        entry(["k3", "c1", "later"], NOON + 20 * MINUTE),
    ]);
    let asked = patched(
        request(&lines, &json!([]), &json!([])),
        "rules",
        &json!([{"pattern": "abc[0-9]+", "placeholder": "[x]"}]),
    );
    let reply = call("pack", &asked.to_string()).unwrap().unwrap();
    let body = body_of(&reply);
    let input = input_of(&body);
    let aliases: Vec<&str> = input
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|block| block.get("conversation").and_then(Value::as_str))
        .collect();
    assert_eq!(aliases, ["C1", "C2", "C1"]);
    let first = input.as_array().and_then(|blocks| blocks.first()).unwrap();
    assert_eq!(
        first.get("lines"),
        Some(&json!([{
            "line": "L1",
            "sender": "alice",
            "text": "token [x]",
            "time_utc": "2026-07-08 04:00:35",
        }]))
    );
    let layout = reply.get("layout").unwrap();
    let labels: Vec<&str> = layout
        .get("segments")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .filter_map(|segment| segment.get("label").and_then(Value::as_str))
        .collect();
    assert_eq!(labels, ["tok", "sec", "lat"]);
}

#[test]
fn pack_narrows_the_options_to_the_hinted_topics() {
    let mut hinted = entry(["k1", "c1", "hello"], NOON);
    if let Some(map) = hinted.as_object_mut() {
        drop(map.insert("hints".to_owned(), json!(["t4-0"])));
    }
    let topics = json!([
        {"label": "first", "topic": "t3-0"},
        {"label": "second", "topic": "t4-0"},
    ]);
    let reply = pack(&json!([hinted]), &json!([]), &topics)
        .unwrap()
        .unwrap();
    let body = body_of(&reply);
    assert_eq!(
        body.pointer("/questions/1/choices"),
        Some(&json!([
            {"description": "second", "value": "t4-0"},
            {"description": "new topic", "value": "new"},
        ]))
    );
}

#[test]
fn pack_offers_earlier_segments_of_the_batch() {
    let lines = json!([
        entry(["k2", "c2", "second"], NOON + 10_000),
        entry(["k1", "c1", "first"], NOON),
        entry(["k3", "c1", "again"], NOON + MINUTE),
    ]);
    let reply = pack(&lines, &json!([]), &json!([])).unwrap().unwrap();
    let body = body_of(&reply);
    assert_eq!(
        body.get("questions")
            .and_then(Value::as_array)
            .and_then(|all| all.last()),
        Some(&json!({
            "choices": [
                {"description": "fir", "value": "S1"},
                {"description": "new topic", "value": "new"},
            ],
            "instructions": "which S2",
            "name": "topic_2",
            "type": "choice",
        }))
    );
    let layout = reply.get("layout").unwrap();
    let keys: Vec<&str> = layout
        .get("lines")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .filter_map(|line| line.get("key").and_then(Value::as_str))
        .collect();
    assert_eq!(keys, ["k1", "k3", "k2"]);
}

#[test]
fn pack_rejects_invalid_rules_and_malformed_requests() {
    let lines = json!([entry(["k1", "c1", "hello"], NOON)]);
    let invalid = patched(
        request(&lines, &json!([]), &json!([])),
        "rules",
        &json!([{"pattern": "(", "placeholder": "x"}]),
    );
    assert_eq!(call("pack", &invalid.to_string()).unwrap(), Err(400));
    assert_eq!(call("pack", "nope").unwrap(), Err(400));
    assert_eq!(call("pack", "{}").unwrap(), Err(400));
}

#[test]
fn pack_skips_the_topic_question_without_two_options() {
    let lines = json!([entry(["k1", "c1", "hello"], NOON)]);
    let reply = pack(&lines, &json!([]), &json!([])).unwrap().unwrap();
    let body = body_of(&reply);
    assert_eq!(
        body.get("questions"),
        Some(&json!([
            {"instructions": "urgent L1", "name": "urgent_1", "type": "predicate"},
        ]))
    );
    assert_eq!(body.get("model"), Some(&json!("m")));
}

#[test]
fn pack_splits_one_conversation_at_long_pauses() {
    let lines = json!([
        entry(["k1", "c1", "a"], NOON),
        entry(["k2", "c1", "b"], NOON + MINUTE),
        entry(["k3", "c1", "c"], NOON + 10 * MINUTE),
    ]);
    let reply = pack(&lines, &json!([]), &json!([])).unwrap().unwrap();
    let layout = reply.get("layout").unwrap();
    let segments: Vec<u64> = layout
        .get("lines")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .filter_map(|line| line.get("segment").and_then(Value::as_u64))
        .collect();
    assert_eq!(segments, [0, 0, 1]);
}

fn request(lines: &Value, attach: &Value, topics: &Value) -> Value {
    json!({
        "attach": attach,
        "batch_id": 7,
        "gap_ms": WINDOW,
        "label_chars": 3,
        "lines": lines,
        "max_chars": ROOMY,
        "model": "m",
        "new_label": "new topic",
        "questions": {
            "topic": {"instructions": "which {segment}", "name": "topic"},
            "urgent": {"instructions": "urgent {line}", "name": "urgent"},
        },
        "rules": [],
        "topics": topics,
    })
}
