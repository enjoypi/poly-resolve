use serde_json::{Error, Value, json};

use super::{Reply, call};

const BAD: f64 = 1.5;
const FIRST_MS: u64 = 1000;
const HIGH: f64 = 0.9;
const LOW: f64 = 0.1;
const MID: f64 = 0.5;
const MIN_CONFIDENCE: f64 = 0.6;
const STRONG: f64 = 0.9;
const THIRD_MS: u64 = 3000;
const WEAK: f64 = 0.2;

fn answers(replies: &[Value]) -> String {
    json!({"answers": replies}).to_string()
}

fn choice(value: &Value, confidence: f64) -> Value {
    json!({"choice": value, "confidence": confidence, "name": "x", "type": "choice"})
}

fn layout() -> Value {
    json!({
        "batch_id": 7,
        "lines": [
            {"key": "k1", "occurred_at_ms": 1000, "segment": 0},
            {"key": "k2", "occurred_at_ms": 2000, "segment": 1},
            {"key": "k3", "occurred_at_ms": 3000, "segment": 2},
        ],
        "segments": [
            {"attach": "t-old", "choices": [], "label": "first", "question": null},
            {
                "attach": null,
                "choices": [
                    {"segment": null, "topic": "t-open", "value": "t-open"},
                    {"segment": 0, "topic": null, "value": "S1"},
                    {"segment": null, "topic": null, "value": "new"},
                ],
                "label": "second",
                "question": 3,
            },
            {
                "attach": null,
                "choices": [
                    {"segment": 1, "topic": null, "value": "S2"},
                    {"segment": null, "topic": null, "value": "new"},
                ],
                "label": "third",
                "question": 4,
            },
        ],
    })
}

fn linked_layout() -> Value {
    json!({
        "batch_id": 7,
        "lines": [
            {"key": "k1", "occurred_at_ms": 1000, "segment": 0},
            {"key": "k2", "occurred_at_ms": 2000, "segment": 1},
        ],
        "segments": [
            {"attach": "t-a", "choices": [], "label": "first", "linked": ["t-a", "t-b"], "question": null},
            {
                "attach": null,
                "choices": [
                    {"segment": 0, "topic": null, "value": "S1"},
                    {"segment": null, "topic": null, "value": "new"},
                ],
                "label": "second",
                "question": 2,
            },
        ],
    })
}

fn predicate(probability: f64) -> Value {
    json!({"name": "x", "probability": probability, "type": "predicate"})
}

fn sort(response: &str) -> Result<Reply, Error> {
    sort_with(&layout(), response)
}

#[test]
fn sort_accepts_null_answers() {
    let response = answers(&[
        Value::Null,
        predicate(LOW),
        predicate(LOW),
        choice(&json!("S2"), STRONG),
        Value::Null,
    ]);
    let reply = sort(&response).unwrap().unwrap();
    assert_eq!(topics_of(&reply), ["t-old", "t7-1", "t7-2"]);
    let placements = reply.get("placements").and_then(Value::as_array).unwrap();
    assert_eq!(
        placements.first().and_then(|item| item.get("probability")),
        Some(&Value::Null)
    );
    assert_eq!(
        placements.get(1).and_then(|item| item.get("probability")),
        Some(&json!(LOW))
    );
}

#[test]
fn sort_falls_back_to_new_topics() {
    let weak = answers(&[
        predicate(LOW),
        predicate(LOW),
        predicate(LOW),
        choice(&json!("t-open"), WEAK),
        choice(&json!("S2"), STRONG),
    ]);
    let reply = sort(&weak).unwrap().unwrap();
    assert_eq!(topics_of(&reply), ["t-old", "t7-1", "t7-1"]);
    assert_eq!(
        reply.get("relations"),
        Some(&json!([{"fresh": "t7-1", "known": "t-open"}]))
    );
    for (first, second) in [
        (json!("zzz"), json!("new")),
        (json!(true), json!("S2")),
        (json!(null), json!("S2")),
    ] {
        let response = answers(&[
            predicate(LOW),
            predicate(LOW),
            predicate(LOW),
            choice(&first, STRONG),
            choice(&second, STRONG),
        ]);
        let found = sort(&response).unwrap().unwrap();
        let topics = topics_of(&found);
        assert_eq!(topics.get(1), Some(&"t7-1"));
        assert!(matches!(topics.get(2), Some(&"t7-1" | &"t7-2")));
    }
}

#[test]
fn sort_groups_lines_by_topic_with_the_highest_probability() {
    let response = answers(&[
        predicate(HIGH),
        predicate(WEAK),
        predicate(MID),
        choice(&json!("t-open"), STRONG),
        choice(&json!("S2"), STRONG),
    ]);
    let reply = sort(&response).unwrap().unwrap();
    assert_eq!(topics_of(&reply), ["t-old", "t-open", "t-open"]);
    assert_eq!(
        reply.get("topics"),
        Some(&json!([
            {"label": "first", "last_ms": FIRST_MS, "probability": HIGH, "topic": "t-old"},
            {"label": "second", "last_ms": THIRD_MS, "probability": MID, "topic": "t-open"},
        ]))
    );
}

#[test]
fn sort_keeps_historical_probability() {
    let mut historical = layout();
    drop(historical.as_object_mut().unwrap().insert(
        "probabilities".to_owned(),
        json!({"t-old": HIGH, "t-open": HIGH}),
    ));
    let response = answers(&[
        predicate(LOW),
        predicate(LOW),
        predicate(LOW),
        choice(&json!("t-open"), STRONG),
        choice(&json!("S2"), STRONG),
    ]);
    let request = json!({
        "layout": historical,
        "min_confidence": MIN_CONFIDENCE,
        "response": response,
    });
    let reply = call("sort", &request.to_string()).unwrap().unwrap();
    let topics = reply.get("topics").and_then(Value::as_array).unwrap();
    for topic in topics {
        assert_eq!(topic.get("probability"), Some(&json!(HIGH)));
    }
}

#[test]
fn sort_leaves_automatic_segments_unplaced_without_a_topic() {
    let none = answers(&[
        predicate(HIGH),
        choice(&json!("none"), STRONG),
        choice(&json!("S2"), STRONG),
    ]);
    let reply = sort_with(&tagged_layout(), &none).unwrap().unwrap();
    assert_eq!(
        reply.get("placements"),
        Some(&json!([
            {"key": "k1", "probability": null, "topic": "t7-0", "via": "new"},
            {"key": "k2", "probability": HIGH, "topic": "t7-0", "via": "new"},
            {"key": "k3", "probability": null, "topic": null, "via": null},
            {"key": "k4", "probability": null, "topic": null, "via": null},
        ]))
    );
    let topics = reply.get("topics").and_then(Value::as_array).unwrap();
    assert_eq!(topics.len(), 1);
    let unsure = answers(&[
        predicate(LOW),
        choice(&json!("t-open"), WEAK),
        choice(&json!("S2"), STRONG),
    ]);
    let loose = sort_with(&tagged_layout(), &unsure).unwrap().unwrap();
    assert_eq!(topics_of(&loose), ["t7-0", "t7-0"]);
    assert_eq!(loose.get("relations"), Some(&json!([])));
    let known = answers(&[
        predicate(LOW),
        choice(&json!("t-open"), STRONG),
        choice(&json!("S2"), STRONG),
    ]);
    let placed = sort_with(&tagged_layout(), &known).unwrap().unwrap();
    assert_eq!(topics_of(&placed), ["t7-0", "t7-0", "t-open", "t-open"]);
    let extra = answers(&[predicate(LOW), predicate(LOW), Value::Null, Value::Null]);
    assert_eq!(sort_with(&tagged_layout(), &extra).unwrap(), Err(400));
}

#[test]
fn sort_links_segments_into_every_linked_topic() {
    let layout = linked_layout();
    let response = answers(&[
        predicate(HIGH),
        predicate(LOW),
        choice(&json!("S1"), STRONG),
    ]);
    let reply = sort_with(&layout, &response).unwrap().unwrap();
    assert_eq!(
        reply.get("placements"),
        Some(&json!([
            {"key": "k1", "probability": HIGH, "topic": "t-a", "via": "adjacent"},
            {"key": "k1", "probability": HIGH, "topic": "t-b", "via": "clue"},
            {"key": "k2", "probability": LOW, "topic": "t-a", "via": "choice"},
            {"key": "k2", "probability": LOW, "topic": "t-b", "via": "choice"},
        ]))
    );
    let unsure = answers(&[predicate(HIGH), predicate(LOW), choice(&json!("S1"), WEAK)]);
    let loose = sort_with(&layout, &unsure).unwrap().unwrap();
    assert_eq!(
        loose.get("relations"),
        Some(&json!([{"fresh": "t7-1", "known": "t-a"}, {"fresh": "t7-1", "known": "t-b"}]))
    );
}

#[test]
fn sort_picks_a_new_topic_for_each_new_choice() {
    let response = answers(&[
        predicate(LOW),
        predicate(LOW),
        predicate(LOW),
        choice(&json!("new"), STRONG),
        choice(&json!("new"), STRONG),
    ]);
    let reply = sort(&response).unwrap().unwrap();
    assert_eq!(topics_of(&reply), ["t-old", "t7-1", "t7-2"]);
}

#[test]
fn sort_rejects_answers_of_the_wrong_type() {
    for (position, answer) in [
        (0, choice(&json!("x"), STRONG)),
        (3, predicate(LOW)),
        (0, json!({"name": "x", "type": "mystery"})),
        (0, json!({"name": "x"})),
    ] {
        let items: Vec<Value> = [
            predicate(LOW),
            predicate(LOW),
            predicate(LOW),
            choice(&json!("new"), STRONG),
            choice(&json!("new"), STRONG),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, item)| {
            if index == position {
                answer.clone()
            } else {
                item
            }
        })
        .collect();
        let response = answers(&items);
        assert_eq!(sort(&response).unwrap(), Err(400), "{position}");
    }
}

#[test]
fn sort_rejects_unusable_responses() {
    for response in ["nope", "{}", r#"{"answers":[]}"#] {
        assert_eq!(sort(response).unwrap(), Err(400), "{response}");
    }
    assert_eq!(call("sort", "nope").unwrap(), Err(400));
    assert_eq!(call("sort", "{}").unwrap(), Err(400));
}

#[test]
fn sort_returns_review_verdicts_by_position() {
    let mut reviewed = layout();
    drop(
        reviewed
            .as_object_mut()
            .unwrap()
            .insert("reviews".to_owned(), json!([{"a": "x"}, {"a": "y"}])),
    );
    for segment in reviewed
        .get_mut("segments")
        .and_then(Value::as_array_mut)
        .unwrap()
    {
        let shifted = segment
            .get("question")
            .and_then(Value::as_u64)
            .map(|slot| slot + 2);
        drop(
            segment
                .as_object_mut()
                .unwrap()
                .insert("question".to_owned(), json!(shifted)),
        );
    }
    let response = answers(&[
        predicate(LOW),
        predicate(LOW),
        predicate(LOW),
        predicate(HIGH),
        Value::Null,
        choice(&json!("new"), STRONG),
        choice(&json!("new"), STRONG),
    ]);
    let reply = sort_with(&reviewed, &response).unwrap().unwrap();
    assert_eq!(
        reply.get("reviews"),
        Some(&json!([
            {"probability": HIGH, "tag": {"a": "x"}},
            {"probability": null, "tag": {"a": "y"}},
        ]))
    );
    assert_eq!(topics_of(&reply), ["t-old", "t7-1", "t7-2"]);
    let short = answers(&[
        predicate(LOW),
        predicate(LOW),
        predicate(LOW),
        choice(&json!("new"), STRONG),
        choice(&json!("new"), STRONG),
    ]);
    assert_eq!(sort_with(&reviewed, &short).unwrap(), Err(400));
}

#[test]
fn sort_treats_refusals_missing_fields_and_bad_probabilities_as_unanswered() {
    let response = answers(&[
        json!({"name": null, "type": "refusal"}),
        predicate(BAD),
        json!({"name": "x", "type": "predicate"}),
        json!({"name": "x", "type": "refusal"}),
        json!({"name": "x", "type": "choice"}),
    ]);
    let reply = sort(&response).unwrap().unwrap();
    assert_eq!(topics_of(&reply), ["t-old", "t7-1", "t7-2"]);
    let placements = reply.get("placements").and_then(Value::as_array).unwrap();
    for placement in placements {
        assert_eq!(placement.get("probability"), Some(&Value::Null));
    }
}

fn sort_with(layout: &Value, response: &str) -> Result<Reply, Error> {
    let request = json!({
        "layout": layout,
        "min_confidence": MIN_CONFIDENCE,
        "response": response,
    });
    call("sort", &request.to_string())
}

fn tagged_layout() -> Value {
    json!({
        "batch_id": 7,
        "lines": [
            {"key": "k1", "occurred_at_ms": 1000, "role": "own", "segment": 0},
            {"key": "k2", "occurred_at_ms": 2000, "segment": 0},
            {"key": "k3", "occurred_at_ms": 3000, "role": "auto", "segment": 1},
            {"key": "k4", "occurred_at_ms": 4000, "role": "auto", "segment": 2},
        ],
        "segments": [
            {"attach": null, "choices": [], "label": "first", "question": null},
            {
                "attach": null,
                "choices": [
                    {"segment": null, "topic": "t-open", "value": "t-open"},
                    {"segment": null, "topic": null, "value": "none"},
                ],
                "label": "second",
                "question": 1,
                "role": "auto",
            },
            {
                "attach": null,
                "choices": [
                    {"segment": 1, "topic": null, "value": "S2"},
                    {"segment": null, "topic": null, "value": "none"},
                ],
                "label": "third",
                "question": 2,
                "role": "auto",
            },
        ],
    })
}

fn topics_of(reply: &Value) -> Vec<&str> {
    reply
        .get("placements")
        .and_then(Value::as_array)
        .map_or_default(|items| {
            items
                .iter()
                .filter_map(|item| item.get("topic").and_then(Value::as_str))
                .collect()
        })
}
