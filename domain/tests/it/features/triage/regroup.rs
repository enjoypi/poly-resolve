use serde_json::{Value, json};

use super::call;

const BIG: usize = 5;
const NONE: usize = 0;
const SMALL: usize = 2;

fn candidate(kind: &str, pair: [&str; 2], held: (&Value, usize)) -> Value {
    let [from, into] = pair;
    let (keys, size) = held;
    json!({"from": from, "into": into, "keys": keys, "kind": kind, "size": size})
}

fn regroup(candidates: &Value) -> Value {
    call("regroup", &json!({"candidates": candidates}).to_string())
        .ok()
        .and_then(Result::ok)
        .and_then(|reply| reply.get("changes").cloned())
        .unwrap_or_default()
}

#[test]
fn regroup_merges_majorities_and_moves_minorities() {
    let changes = regroup(&json!([
        candidate("clue", ["b", "a"], (&json!(["k1"]), SMALL)),
        candidate("clue", ["d", "c"], (&json!(["k2"]), BIG)),
        candidate("vote", ["f", "e"], (&json!([]), NONE)),
    ]));
    assert_eq!(
        changes,
        json!([
            {"from": "b", "into": "a", "keys": [], "kind": "merge", "reason": "clue"},
            {"from": "d", "into": "c", "keys": ["k2"], "kind": "move", "reason": "clue"},
            {"from": "f", "into": "e", "keys": [], "kind": "merge", "reason": "vote"},
        ])
    );
}

#[test]
fn regroup_rejects_malformed_request() {
    assert_eq!(call("regroup", "nope").unwrap(), Err(400));
    assert_eq!(call("regroup", "{}").unwrap(), Err(400));
}

#[test]
fn regroup_splits_parts_and_skips_whole_or_empty_ones() {
    let changes = regroup(&json!([
        candidate("apart", ["a", ""], (&json!(["k1"]), BIG)),
        candidate("apart", ["b", ""], (&json!(["k2", "k3"]), SMALL)),
        candidate("apart", ["c", ""], (&json!([]), BIG)),
        candidate("apart", ["d", ""], (&json!(["k4"]), BIG)),
    ]));
    assert_eq!(
        changes,
        json!([
            {"from": "a", "into": "", "keys": ["k1"], "kind": "split", "reason": "apart"},
            {"from": "d", "into": "", "keys": ["k4"], "kind": "split", "reason": "apart"},
        ])
    );
}

#[test]
fn regroup_touches_each_topic_once_per_round() {
    let changes = regroup(&json!([
        candidate("vote", ["b", "a"], (&json!([]), NONE)),
        candidate("vote", ["c", "b"], (&json!([]), NONE)),
        candidate("vote", ["d", "a"], (&json!([]), NONE)),
        candidate("apart", ["a", ""], (&json!(["k1"]), BIG)),
    ]));
    assert_eq!(
        changes,
        json!([{"from": "b", "into": "a", "keys": [], "kind": "merge", "reason": "vote"}])
    );
}
