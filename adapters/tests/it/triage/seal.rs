use adapters::store::Archive;
use serde_json::{Value, json};

use super::rig::{
    KEY, LATER, QUICK, SECOND, THIRD, decided, input_of, logged, seed, seed_from, start, table,
};
use crate::support::{BUSY, NOON, Sandbox};

const COPIES: u64 = 2;
const FORWARDED: &str = "forwarded announcement text";
const HASH_CHARS: usize = 8;
const PEOPLE: &str =
    "SELECT source || '|' || sender_id || '|' || person_id AS item FROM identities ORDER BY 1";
const REACH: &str = "SELECT reach AS item FROM topics ORDER BY id";
const TEXTS: &str = "SELECT text AS item FROM lines ORDER BY key";
const VIA: &str = "SELECT key || '|' || topic_id || '|' || via AS item FROM members ORDER BY key";

#[tokio::test]
async fn forwarded_copies_are_sent_once_and_counted_once() {
    let sandbox = Sandbox::new("seal-copies").unwrap();
    seed(
        &sandbox,
        &[
            (KEY, "u1", "c1", FORWARDED),
            (SECOND, "u2", "c2", FORWARDED),
        ],
    )
    .unwrap();
    let patch = json!({"stub": {"decide": {"p": "0.1", "topics": "x S1"}}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    let input = input_of(&decided(&sandbox).unwrap()).unwrap();
    assert_eq!(input.pointer("/0/lines/0/copies"), Some(&json!(COPIES)));
    assert_eq!(
        input.pointer("/1/lines/0"),
        Some(&json!({"line": "L2", "repeat": "L1"}))
    );
    assert_eq!(
        table(&sandbox, REACH).unwrap(),
        [r#"{"conversations":1,"people":1,"sources":1}"#]
    );
    seed_from(&sandbox, &[(THIRD, "u3", "c3", FORWARDED)], LATER).unwrap();
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, VIA).unwrap(),
        [
            format!("{KEY}|t1-0|new"),
            format!("{SECOND}|t1-0|choice"),
            format!("{THIRD}|t1-0|clue"),
        ]
    );
    assert_eq!(
        table(&sandbox, REACH).unwrap(),
        [r#"{"conversations":1,"people":1,"sources":1}"#]
    );
}

#[tokio::test]
async fn hashed_placeholders_are_stable_across_lines() {
    let sandbox = Sandbox::new("seal-hash").unwrap();
    seed(
        &sandbox,
        &[
            (KEY, "u1", "c1", "call 13812345678"),
            (SECOND, "u1", "c1", "again 13812345678 or 13900000000"),
        ],
    )
    .unwrap();
    let patch = json!({
        "redact": {"patterns": [{"pattern": "[0-9]{11,}", "placeholder": "[num#{hash}]"}]},
        "stub": {"decide": {"p": "0.1"}},
    });
    start(&sandbox, &patch, QUICK).await.unwrap();
    let texts = table(&sandbox, TEXTS).unwrap();
    let first = texts.first().map_or("", String::as_str);
    let tag = first.strip_prefix("call ").unwrap_or_default();
    assert!(tag.starts_with("[num#") && tag.len() == "[num#]".len() + HASH_CHARS);
    let second = texts.get(1).map_or("", String::as_str);
    let rest = second
        .strip_prefix(&format!("again {tag} or "))
        .unwrap_or_default();
    assert!(rest.starts_with("[num#") && rest != tag);
    let input = input_of(&decided(&sandbox).unwrap()).unwrap();
    assert_eq!(
        input.pointer("/0/lines/0/text").and_then(Value::as_str),
        Some(first)
    );
}

#[tokio::test]
async fn one_person_on_two_sources_counts_once() {
    let sandbox = Sandbox::new("seal-people").unwrap();
    let archive = Archive::open(&sandbox.database(), BUSY).unwrap();
    for (key, source, sender, name, conversation) in [
        (KEY, "dingtalk", "d1", "Alice Wang", "c1"),
        ("imap:acct:m9", "imap", "a@x.test", "alice  wang", "c2"),
    ] {
        let envelope = json!({
            "account": "acct",
            "conversation_id": conversation,
            "key": key,
            "occurred_at_ms": NOON,
            "raw": "{}",
            "received_at_ms": NOON,
            "sender_id": sender,
            "sender_name": name,
            "source": source,
            "text": "server down",
        });
        let _: usize = archive.save_event(&envelope.to_string()).unwrap();
    }
    let patch = json!({"stub": {"decide": {"p": "0.1", "topics": "x S1"}}});
    let logs = logged(&sandbox, &patch, QUICK).await.unwrap();
    assert!(logs.contains("senders identified"));
    assert_eq!(
        table(&sandbox, PEOPLE).unwrap(),
        [
            "dingtalk|d1|name:alice wang",
            "imap|a@x.test|name:alice wang"
        ]
    );
    assert_eq!(
        table(&sandbox, REACH).unwrap(),
        [r#"{"conversations":2,"people":1,"sources":2}"#]
    );
}
