use serde_json::{Error, Value, json};

use super::{Reply, call};

fn people(senders: &Value) -> Result<Reply, Error> {
    call("people", &json!({"senders": senders}).to_string())
}

#[test]
fn people_link_unique_names_across_sources() {
    let senders = json!([
        {"name": "Alice  Wang", "sender_id": "d1", "source": "dingtalk"},
        {"name": "alice wang", "sender_id": "a@x.test", "source": "imap"},
        {"name": "Bob", "sender_id": "d2", "source": "dingtalk"},
        {"name": "", "sender_id": "d3", "source": "dingtalk"},
        {"name": "", "sender_id": "n@x.test", "source": "imap"},
    ]);
    let reply = people(&senders).unwrap().unwrap();
    assert_eq!(
        reply.pointer("/identities/0"),
        Some(&json!({
            "name": "alice wang",
            "person_id": "name:alice wang",
            "sender_id": "d1",
            "source": "dingtalk",
        }))
    );
    for (index, person) in [
        "name:alice wang",
        "name:alice wang",
        "dingtalk:d2",
        "dingtalk:d3",
        "imap:n@x.test",
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            reply.pointer(&format!("/identities/{index}/person_id")),
            Some(&json!(person))
        );
    }
}

#[test]
fn people_reject_malformed_request() {
    assert_eq!(call("people", "nope").unwrap(), Err(400));
    assert_eq!(call("people", "{}").unwrap(), Err(400));
}

#[test]
fn people_with_clashing_names_stay_apart() {
    let senders = json!([
        {"name": "Alice", "sender_id": "d1", "source": "dingtalk"},
        {"name": "Alice", "sender_id": "a@x.test", "source": "imap"},
        {"name": "alice", "sender_id": "b@x.test", "source": "imap"},
    ]);
    let reply = people(&senders).unwrap().unwrap();
    for (index, person) in ["dingtalk:d1", "imap:a@x.test", "imap:b@x.test"]
        .into_iter()
        .enumerate()
    {
        assert_eq!(
            reply.pointer(&format!("/identities/{index}/person_id")),
            Some(&json!(person))
        );
    }
}
