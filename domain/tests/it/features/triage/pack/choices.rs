use serde_json::{Value, json};

use super::{
    COPIES, MINUTE, NO_PREDICATES, NOON, body_of, call, entry, input_of, pack, patched, request,
    tagged,
};

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
fn pack_offers_no_new_topic_to_automatic_segments() {
    let lines = json!([tagged(entry(["k1", "c1", "alarm"], NOON), "auto")]);
    let topics = json!([{"label": "old", "topic": "t3-0"}]);
    let reply = pack(&lines, &json!([]), &topics).unwrap().unwrap();
    assert_eq!(
        body_of(&reply).get("questions"),
        Some(&json!([
            {
                "choices": [
                    {"description": "old", "value": "t3-0"},
                    {"description": "no topic", "value": "none"},
                ],
                "instructions": "which S1",
                "name": "topic_1",
                "type": "choice",
            },
        ]))
    );
    let layout = reply.get("layout").unwrap();
    assert_eq!(layout.pointer("/segments/0/role"), Some(&json!("auto")));
    assert_eq!(
        layout.pointer("/segments/0/question"),
        Some(&json!(NO_PREDICATES))
    );
    let alone = pack(&lines, &json!([]), &json!([])).unwrap().unwrap();
    assert_eq!(body_of(&alone).get("questions"), Some(&json!([])));
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
fn pack_sends_one_copy_of_repeated_lines() {
    let copy = |entry: Value| patched(entry, "digest", &json!("d1"));
    let lines = json!([
        copy(entry(["k1", "c1", "forwarded"], NOON)),
        copy(entry(["k2", "c2", "forwarded"], NOON + MINUTE)),
        entry(["k3", "c2", "other"], NOON + MINUTE),
    ]);
    let reply = pack(&lines, &json!([]), &json!([])).unwrap().unwrap();
    let body = body_of(&reply);
    let input = input_of(&body);
    assert_eq!(input.pointer("/0/lines/0/copies"), Some(&json!(COPIES)));
    assert_eq!(
        input.pointer("/1/lines/0"),
        Some(&json!({"line": "L2", "repeat": "L1"}))
    );
    assert_eq!(input.pointer("/1/lines/1/copies"), None);
    let layout = reply.get("layout").unwrap();
    assert_eq!(layout.pointer("/lines/1/role"), Some(&json!("copy")));
    let names: Vec<&str> = body
        .get("questions")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .filter_map(|question| question.get("name").and_then(Value::as_str))
        .collect();
    assert_eq!(names, ["urgent_1", "urgent_3", "topic_2"]);
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
