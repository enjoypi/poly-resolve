use adapters::store::Archive;
use serde_json::json;

use super::rig::{LINES, PLACEMENTS, QUICK, decided, questions_of, table};
use crate::imap::settings;
use crate::support::{BUSY, NOON, Run, Sandbox};

#[tokio::test]
async fn own_and_automatic_mail_is_tagged() {
    let sandbox = Sandbox::new("triage-mailbox").unwrap();
    let ingest = settings(&sandbox, "imap://127.0.0.1:1", &json!({})).unwrap();
    let archive = Archive::open(&sandbox.database(), BUSY).unwrap();
    for (key, sender, raw) in [
        ("imap:work:m1", "ME@x.test", "{}"),
        ("imap:work:m2", "other@x.test", "{}"),
        ("imap:work:m3", "news@x.test", r#"{"bulk":"list"}"#),
        ("imap:work:m4", "odd@x.test", "not json"),
    ] {
        let envelope = json!({
            "account": "work",
            "conversation_id": format!("mail:work:{sender}"),
            "key": key,
            "occurred_at_ms": NOON,
            "raw": raw,
            "received_at_ms": NOON,
            "sender_id": sender,
            "sender_name": "name",
            "source": "imap",
            "text": "hello",
        });
        let _: usize = archive.save_event(&envelope.to_string()).unwrap();
    }
    sandbox.write("decide-stub.json", r#"{"p":"0.1"}"#).unwrap();
    let triage = sandbox.triage(&json!({})).unwrap();
    drop(
        sandbox
            .triage_logged(&Run::new(&ingest, &triage), QUICK)
            .await
            .unwrap(),
    );
    assert_eq!(
        table(&sandbox, LINES).unwrap(),
        [
            "imap:work:m1|approved||1",
            "imap:work:m2|approved||1",
            "imap:work:m3|unrelated|no_topic|1",
            "imap:work:m4|approved||1",
        ]
    );
    assert_eq!(
        table(&sandbox, "SELECT role AS item FROM lines ORDER BY key").unwrap(),
        ["own", "", "auto", ""]
    );
    let body = decided(&sandbox).unwrap();
    assert_eq!(
        questions_of(&body),
        ["urgent_2", "urgent_4", "topic_2", "topic_3", "topic_4"]
    );
    assert_eq!(
        body.pointer("/questions/3/choices/2/value"),
        Some(&json!("none"))
    );
    let placed = table(&sandbox, PLACEMENTS).unwrap();
    assert_eq!(placed.len(), 3);
    assert!(!placed.concat().contains("imap:work:m3"));
}
