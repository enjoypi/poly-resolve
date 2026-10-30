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
const THRESHOLD: f64 = 0.8;
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

fn predicate(probability: f64) -> Value {
    json!({"name": "x", "probability": probability, "type": "predicate"})
}

fn sort(response: &str) -> Result<Reply, Error> {
    let request = json!({
        "layout": layout(),
        "min_confidence": MIN_CONFIDENCE,
        "response": response,
        "threshold": THRESHOLD,
    });
    call("sort", &request.to_string())
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
            {"label": "first", "last_ms": FIRST_MS, "probability": HIGH, "topic": "t-old", "urgent": "urgent"},
            {"label": "second", "last_ms": THIRD_MS, "probability": MID, "topic": "t-open", "urgent": "normal"},
        ]))
    );
}

#[test]
fn sort_keeps_historical_topic_probability_when_new_lines_score_lower() {
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
        "threshold": THRESHOLD,
    });
    let reply = call("sort", &request.to_string()).unwrap().unwrap();
    let topics = reply.get("topics").and_then(Value::as_array).unwrap();
    for topic in topics {
        assert_eq!(topic.get("probability"), Some(&json!(HIGH)));
        assert_eq!(topic.get("urgent"), Some(&json!("urgent")));
    }
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
    let topics = reply.get("topics").and_then(Value::as_array).unwrap();
    for topic in topics {
        assert_eq!(topic.get("urgent"), Some(&json!("normal")));
    }
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
