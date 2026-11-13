use serde_json::{Value, json};

use super::call;

const FIVE: u64 = 5;
const FOUR: u64 = 4;
const NINE: u64 = 9;
const ONE: u64 = 1;
const SEVEN: u64 = 7;
const TWO: u64 = 2;
const ZERO: u64 = 0;

#[test]
fn spent_reads_whole_body_or_event_lines() {
    let body = json!({"answers": [], "usage": usage(SEVEN, TWO)}).to_string();
    let found = call("spent", &json!({"text": body}).to_string())
        .unwrap()
        .unwrap();
    assert_eq!(found, usage(SEVEN, TWO));
    let lines = [
        json!({"type": "thread.started"}).to_string(),
        json!({"type": "turn.completed", "usage": usage(FIVE, ONE)}).to_string(),
        "not json".to_owned(),
        json!({"type": "turn.completed", "usage": {"input_tokens": FOUR}}).to_string(),
        json!({"type": "turn.completed", "usage": "bad"}).to_string(),
    ];
    let stream = json!({"text": lines.join("\n")});
    let summed = call("spent", &stream.to_string()).unwrap().unwrap();
    assert_eq!(summed, usage(NINE, ONE));
    let bare = json!({"text": "{}"});
    let none = call("spent", &bare.to_string()).unwrap().unwrap();
    assert_eq!(none, usage(ZERO, ZERO));
    assert_eq!(call("spent", "nope").unwrap(), Err(400));
}

fn usage(input: u64, output: u64) -> Value {
    json!({"input_tokens": input, "output_tokens": output})
}
