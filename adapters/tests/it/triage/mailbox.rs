use adapters::store::Archive;
use serde_json::json;

use super::rig::{LINES, QUICK, table};
use crate::imap::settings;
use crate::support::{BUSY, NOON, Run, Sandbox};

#[tokio::test]
async fn own_mail_is_skipped_by_any_listed_address() {
    let sandbox = Sandbox::new("triage-mailbox").unwrap();
    let ingest = settings(&sandbox, "imap://127.0.0.1:1", &json!({})).unwrap();
    let archive = Archive::open(&sandbox.database(), BUSY).unwrap();
    for (key, sender) in [
        ("imap:work:m1", "ME@x.test"),
        ("imap:work:m2", "other@x.test"),
    ] {
        let envelope = json!({
            "account": "work",
            "conversation_id": format!("mail:work:{sender}"),
            "key": key,
            "occurred_at_ms": NOON,
            "raw": "{}",
            "received_at_ms": NOON,
            "sender_id": sender,
            "sender_name": "name",
            "source": "imap",
            "text": "hello",
        });
        let _: usize = archive.save_event(&envelope.to_string()).unwrap();
    }
    let triage = sandbox.triage(&json!({})).unwrap();
    drop(
        sandbox
            .triage_logged(&Run::new(&ingest, &triage), QUICK)
            .await
            .unwrap(),
    );
    assert_eq!(
        table(&sandbox, LINES).unwrap(),
        ["imap:work:m1|skipped|own|0", "imap:work:m2|approved||1"]
    );
}
