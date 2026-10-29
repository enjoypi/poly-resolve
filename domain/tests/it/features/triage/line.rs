use serde_json::{Error, Value, json};

use super::{Reply, call, patched};

const BODY: &str = "QUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVo0MTIzNDU2";
const CIPHER_MIN: u32 = 32;
const NOTICE: &str = "notice-conversation";
const OWN: &str = "me";
const ROOMY: u32 = 10_000;
const SHORT: u32 = 5;
const UNKNOWN: &str = "unknown";

fn body(message: &Value) -> Value {
    json!({
        "cipher_min_length": CIPHER_MIN,
        "max_chars": ROOMY,
        "message": message,
        "notify_conversation_id": NOTICE,
        "rules": [],
        "self_sender_id": OWN,
        "unknown_sender": UNKNOWN,
    })
}

fn line(message: &Value) -> Result<Reply, Error> {
    call("line", &body(message).to_string())
}

#[test]
fn line_fails_closed_on_invalid_rules() {
    let invalid = json!([{"pattern": "(", "placeholder": "x"}]);
    let message = json!({
        "conversation_id": "c1",
        "sender_id": "u1",
        "sender_name": "alice",
        "text": "hello",
    });
    let request = patched(body(&message), "rules", &invalid);
    assert_eq!(call("line", &request.to_string()).unwrap(), Err(400));
    let blank = json!({
        "conversation_id": "c1",
        "sender_id": "u1",
        "sender_name": "alice",
        "text": " ",
    });
    let skipped = patched(body(&blank), "rules", &invalid);
    let reply = call("line", &skipped.to_string()).unwrap().unwrap();
    assert_eq!(reply.get("skip"), Some(&json!("empty")));
}

#[test]
fn line_masks_sender_and_text_and_truncates() {
    let rules = json!([{"pattern": "[0-9]{4}", "placeholder": "[n]"}]);
    let message = json!({
        "conversation_id": "c1",
        "sender_id": "u1",
        "sender_name": "alice 1234",
        "text": "pin 5678 end",
    });
    let masked = call(
        "line",
        &patched(body(&message), "rules", &rules).to_string(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(masked.get("sender"), Some(&json!("alice [n]")));
    assert_eq!(masked.get("text"), Some(&json!("pin [n] end")));
    assert_eq!(masked.get("skip"), Some(&Value::Null));
    let short = patched(
        patched(body(&message), "rules", &rules),
        "max_chars",
        &json!(SHORT),
    );
    let cut = call("line", &short.to_string()).unwrap().unwrap();
    assert_eq!(cut.get("text"), Some(&json!("pin [")));
}

#[test]
fn line_names_unknown_senders() {
    let message = json!({
        "conversation_id": "c1",
        "sender_id": "u1",
        "sender_name": "",
        "text": "hello",
    });
    let reply = line(&message).unwrap().unwrap();
    assert_eq!(reply.get("sender"), Some(&json!(UNKNOWN)));
}

#[test]
fn line_rejects_malformed_request() {
    assert_eq!(call("line", "nope").unwrap(), Err(400));
    assert_eq!(call("line", "{}").unwrap(), Err(400));
}

#[test]
fn line_skips_each_reason() {
    let sealed = format!("{BODY}||2||1||196");
    for (sender, conversation, text, reason) in [
        (OWN, "c1", "hello", "own"),
        ("u1", NOTICE, "hello", "notice"),
        ("u1", "c1", " \n ", "empty"),
        ("u1", "c1", sealed.as_str(), "sealed"),
    ] {
        let message = json!({
            "conversation_id": conversation,
            "sender_id": sender,
            "sender_name": "alice",
            "text": text,
        });
        let reply = line(&message).unwrap().unwrap();
        assert_eq!(reply.get("skip"), Some(&json!(reason)));
        assert_eq!(reply.get("text"), Some(&json!("")));
    }
}
