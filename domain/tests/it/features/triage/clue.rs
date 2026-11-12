use serde_json::{Error, Value, json};

use super::{Reply, call};

const MIN_CHARS: u32 = 4;
const ODD: u32 = 7;

fn clues(message: &Value) -> Result<Reply, Error> {
    let request = json!({
        "message": message,
        "patterns": [
            {"kind": "link", "pattern": "https?://[a-z0-9./-]+"},
            {"kind": "ticket", "pattern": "ticket ([A-Z]+-[0-9]+)"},
        ],
        "subject": {"ignore": ["notice"], "min_chars": MIN_CHARS, "prefixes": ["re:", "fwd:", ""]},
    });
    call("clues", &request.to_string())
}

#[test]
fn clues_come_from_configured_patterns() {
    let text = "see https://x.test/a and https://x.test/a, ticket OPS-12 and ticket OPS-13";
    let reply = clues(&json!({"text": text})).unwrap().unwrap();
    assert_eq!(
        reply,
        json!({"clues": [
            {"kind": "link", "value": "https://x.test/a"},
            {"kind": "ticket", "value": "OPS-12"},
            {"kind": "ticket", "value": "OPS-13"},
        ]})
    );
}

#[test]
fn clues_come_from_mail_headers_and_subject() {
    let raw = json!({
        "in_reply_to": "m0@x.test",
        "message_id": "m1@x.test",
        "references": ["a@x.test", "m0@x.test", ODD],
        "subject": " RE: Fwd: re:  Deploy Plan ",
    });
    let reply = clues(&json!({"raw": raw, "text": "plain"}))
        .unwrap()
        .unwrap();
    assert_eq!(
        reply,
        json!({"clues": [
            {"kind": "mail", "value": "a@x.test"},
            {"kind": "mail", "value": "m0@x.test"},
            {"kind": "mail", "value": "m1@x.test"},
            {"kind": "subject", "value": "deploy plan"},
        ]})
    );
}

#[test]
fn clues_ignore_short_or_generic_subjects() {
    for subject in ["re: hi", "Re: Notice", "re:"] {
        let reply = clues(&json!({"raw": {"subject": subject}, "text": ""}))
            .unwrap()
            .unwrap();
        assert_eq!(reply, json!({"clues": []}), "{subject}");
    }
    let odd = clues(&json!({"raw": "text", "text": ""})).unwrap().unwrap();
    assert_eq!(odd, json!({"clues": []}));
}

#[test]
fn clues_reject_invalid_patterns_and_malformed_requests() {
    let request = json!({
        "message": {"text": "x"},
        "patterns": [{"kind": "link", "pattern": "("}],
        "subject": {"ignore": [], "min_chars": MIN_CHARS, "prefixes": []},
    });
    assert_eq!(call("clues", &request.to_string()).unwrap(), Err(400));
    assert_eq!(call("clues", "nope").unwrap(), Err(400));
    assert_eq!(call("clues", "{}").unwrap(), Err(400));
}
