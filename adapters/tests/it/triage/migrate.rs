use adapters::store::Archive;
use serde_json::json;

use super::rig::{PLACEMENTS, QUICK, start, table};
use crate::support::{BUSY, Sandbox};

const LEGACY: &str = "INSERT INTO lines (key, conversation_id, occurred_at_ms, sender, text, state, reason, reviewer, flag, hints, rules_rev, batch_id, created_at_ms, updated_at_ms) VALUES ('a', 'c1', 1, '', '', 'skipped', 'own', '', '', '[]', 0, 0, 1, 1), ('b', 'c1', 2, 's', 't', 'skipped', 'expired', '', '', '[]', 0, 0, 1, 1), ('c', 'c1', 3, 's', 't', 'pending', '', 'me', '', '[]', 0, 4, 1, 1), ('d', 'c1', 4, 's', 't', 'leaked', '', '', '', '[]', 0, 4, 1, 1), ('e', 'c1', 5, '', '', 'skipped', 'sealed', '', '', '[]', 0, 0, 1, 1), ('f', 'c1', 6, '', '', 'skipped', 'empty', '', '', '[]', 0, 0, 1, 1), ('g', 'c1', 7, 's', 't', 'rejected', 'screen', '', 'unmasked', '[]', 0, 3, 1, 1), ('h', 'c1', 8, 's', 't', 'rejected', 'screen', '', 'unscreened', '[]', 0, 3, 1, 1), ('i', 'c1', 9, 's', 't', 'rejected', 'noise', '', '', '[]', 0, 3, 1, 1), ('j', 'c1', 10, '', '', 'skipped', 'notice', '', '', '[]', 0, 0, 1, 1);";
const MAIL: &str = "INSERT INTO messages (key, source, account, conversation_id, sender_id, sender_name, text, occurred_at_ms, received_at_ms, raw) VALUES ('imap:work:m1', 'imap', 'work', 'mail:work:a@x.test', 'a', 'a', 't', 1, 1, '{\"message_id\":\"m1\"}'), ('imap:work:m2', 'imap', 'work', 'mail:work:a@x.test', 'a', 'a', 't', 2, 2, '{\"in_reply_to\":\"m1\",\"message_id\":\"m2\"}'), ('imap:work:m3', 'imap', 'work', 'mail:work:a@x.test', 'a', 'a', 't', 3, 3, '{\"in_reply_to\":\"m2\",\"references\":[\"m0\",\"m1\"]}'), ('imap:work:uid:7:9', 'imap', 'work', 'mail:work:b@x.test', 'b', 'b', 't', 4, 4, '{}'), ('imap:work:m5', 'imap', 'work', 'mail:work:c@x.test', 'c', 'c', 't', 5, 5, 'junk'); INSERT INTO lines (key, conversation_id, occurred_at_ms, sender, text, state, reason, reviewer, flag, hints, rules_rev, batch_id, created_at_ms, updated_at_ms) VALUES ('imap:work:m1', 'mail:work:a@x.test', 1, 's', 't', 'approved', '', '', '', '[]', 0, 1, 1, 1); INSERT INTO placements VALUES ('imap:work:m1', 't1-0', 0.5, 1); INSERT INTO topics (id, label, probability, thread_id, until_ms, until_key, thread_at_ms, created_at_ms, last_at_ms) VALUES ('t1-0', 'l', 0.5, '', 0, '', 0, 1, 1);";
const MOVED: &str = "SELECT key || '|' || conversation_id AS item FROM messages ORDER BY key";
const RETIRED: &str = "INSERT INTO topics (id, label, probability, thread_id, until_ms, until_key, thread_at_ms, created_at_ms, last_at_ms, purged_at_ms) VALUES ('T1', 'l', 0.5, 'T1', 0, '', 1, 1, 1, 5); INSERT INTO topic_aliases VALUES ('t1-0', 'T1'), ('s3', 'T1'), ('X9', 'T1');";
const SCORED: &str = "SELECT id || '|' || score || '|' || urgency AS item FROM topics";
const STATES: &str = "SELECT key || '|' || state || '|' || reason || '|' || batch_id || '|' || reviewer AS item FROM lines ORDER BY key";

#[tokio::test]
async fn legacy_ledgers_gain_members_and_mail_threads_once() {
    let sandbox = Sandbox::new("migrate-members").unwrap();
    drop(Archive::open(&sandbox.database(), BUSY).unwrap());
    sandbox.seed(MAIL).unwrap();
    let off = json!({"mode": "disabled"});
    for _round in ["first", "second"] {
        start(&sandbox, &off, QUICK).await.unwrap();
        assert_eq!(
            table(&sandbox, MOVED).unwrap(),
            [
                "imap:work:m1|mail:work:thread:m1",
                "imap:work:m2|mail:work:thread:m1",
                "imap:work:m3|mail:work:thread:m0",
                "imap:work:m5|mail:work:c@x.test",
                "imap:work:uid:7:9|mail:work:thread:uid:7:9",
            ]
        );
        assert_eq!(
            table(&sandbox, "SELECT conversation_id AS item FROM lines").unwrap(),
            ["mail:work:thread:m1"]
        );
        assert_eq!(
            table(&sandbox, PLACEMENTS).unwrap(),
            ["imap:work:m1|t1-0|0.5"]
        );
        assert_eq!(table(&sandbox, SCORED).unwrap(), ["t1-0|0.5|0.5"]);
    }
}

#[tokio::test]
async fn legacy_line_states_are_migrated_once() {
    let sandbox = Sandbox::new("migrate-lines").unwrap();
    drop(Archive::open(&sandbox.database(), BUSY).unwrap());
    sandbox.seed(LEGACY).unwrap();
    let off = json!({"mode": "disabled"});
    for _round in ["first", "second"] {
        start(&sandbox, &off, QUICK).await.unwrap();
        assert_eq!(
            table(&sandbox, STATES).unwrap(),
            [
                "c|queued||0|",
                "d|queued||0|",
                "e|withheld|sealed|0|",
                "f|withheld|empty|0|",
                "g|withheld|risky|3|",
                "h|withheld|unscreened|3|",
                "i|rejected|noise|3|",
                "j|skipped|notice|0|",
            ]
        );
    }
}

#[tokio::test]
async fn temporary_topic_ids_are_not_backfilled_as_sessions() {
    let sandbox = Sandbox::new("migrate-threads").unwrap();
    drop(Archive::open(&sandbox.database(), BUSY).unwrap());
    sandbox.seed(RETIRED).unwrap();
    start(&sandbox, &json!({"mode": "disabled"}), QUICK)
        .await
        .unwrap();
    assert_eq!(
        table(
            &sandbox,
            "SELECT id || '|' || topic_id || '|' || purged_at_ms AS item FROM threads ORDER BY id"
        )
        .unwrap(),
        ["T1|T1|5", "X9|T1|0"]
    );
}
