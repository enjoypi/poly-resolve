use adapters::store::{Archive, Shelf};
use domain::outbound::Listing;
use domain::topics::Board as _;
use serde_json::{Value, to_value};

use crate::support::{BUSY, Fallible, Sandbox};

const BROKEN: &str = "INSERT INTO topics (id, label, probability, thread_id, until_ms, until_key, thread_at_ms, created_at_ms, last_at_ms) VALUES ('bad', 'broken', NULL, '', 0, '', 0, 1, 'abc');";
const LIMIT: u32 = 10;
const ROWS: &str = "INSERT INTO topics (id, label, probability, thread_id, until_ms, until_key, thread_at_ms, created_at_ms, last_at_ms, state, urgency, reach, score, verdict) VALUES ('t1', 'first', 0.5, '', 0, '', 0, 1, 5, 'open', 0.5, '{\"conversations\":2,\"people\":3}', 0.75, ''), ('t2', 'second', 0.25, '', 0, '', 0, 1, 9, 'resolved', NULL, '{}', NULL, 'done'); UPDATE topics SET score = 0.5 WHERE id = 't1'; UPDATE topics SET score = 0.5 WHERE id = 't1'; UPDATE topics SET score = 0.75 WHERE id = 't1'; INSERT INTO topic_aliases VALUES ('old', 't1'); INSERT INTO messages (key, source, account, conversation_id, sender_id, sender_name, text, occurred_at_ms, received_at_ms, raw) VALUES ('m1', 'imap', 'hf', 'c1', 's', 'alice', 'raw one', 1, 1, ''), ('m2', 'dingtalk', 'hf', 'c2', 's', 'bob', 'raw two', 2, 2, ''); INSERT INTO lines (key, conversation_id, occurred_at_ms, sender, text, state, reason, reviewer, flag, hints, rules_rev, batch_id, created_at_ms, updated_at_ms, role) VALUES ('m1', 'c1', 1, 'alice', 'line one', 'analysed', '', 'auto', '', '[]', 0, 1, 1, 1, ''), ('m2', 'c2', 2, 'bob', 'secret two', 'withheld', 'screen', '', '', '[]', 0, 1, 2, 2, 'own'), ('m3', 'c3', 3, 'carol', 'line three', 'skipped', 'notice', '', '', '[]', 0, 0, 3, 3, ''); INSERT INTO members VALUES ('m1', 't1', 0.5, 1, 'new', 1), ('m2', 't1', 0.5, 1, 'clue', 2), ('m3', 't2', NULL, 1, 'new', 3); INSERT INTO alerts (key, topic_id, state, attempts, analysis, reason, created_at_ms, updated_at_ms) VALUES ('a1', 't1', 'sent', 0, 'old view', '', 1, 1), ('a2', 't1', 'sent', 0, 'new view', '', 2, 2); INSERT INTO changes (kind, from_topic, to_topic, keys, reason, state, at_ms) VALUES ('move', 't2', 't1', '[\"m2\"]', 'clue', 'applied', 7), ('merge', 't3', 't4', '[]', 'vote', 'applied', 8);";

#[test]
fn detail_fails_on_missing_database() {
    let sandbox = Sandbox::new("topics-detail-missing").unwrap();
    let path = sandbox.database();
    let err = Shelf::new(&path, BUSY).detail("t1").unwrap_err();
    assert_eq!(err.status(), 503);
}

#[test]
fn detail_fails_on_undecodable_row() {
    let sandbox = seeded("topics-detail-undecodable", BROKEN).unwrap();
    let path = sandbox.database();
    let err = Shelf::new(&path, BUSY).detail("bad").unwrap_err();
    assert_eq!(err.status(), 503);
}

#[test]
fn detail_fails_without_tables() {
    let sandbox = Sandbox::new("topics-detail-table").unwrap();
    sandbox.seed("CREATE TABLE other (key TEXT);").unwrap();
    let path = sandbox.database();
    let err = Shelf::new(&path, BUSY).detail("t1").unwrap_err();
    assert_eq!(err.status(), 503);
}

#[test]
fn detail_follows_alias_and_hides_withheld_text() {
    let sandbox = seeded("topics-detail", ROWS).unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, BUSY);
    let found = to_value(shelf.detail("old").unwrap()).unwrap();
    assert_eq!(peek(&found, "/topic/topic_id"), r#""t1""#);
    assert_eq!(peek(&found, "/topic/verdict"), r#""new view""#);
    assert_eq!(peek(&found, "/topic/members"), "2");
    assert_eq!(peek(&found, "/topic/people"), "3");
    assert_eq!(peek(&found, "/timeline/0/text"), r#""line one""#);
    assert_eq!(peek(&found, "/timeline/0/source"), r#""imap""#);
    assert_eq!(peek(&found, "/timeline/1/text"), r#""""#);
    assert_eq!(peek(&found, "/timeline/1/reason"), r#""screen""#);
    assert_eq!(peek(&found, "/timeline/1/via"), r#""clue""#);
    assert_eq!(peek(&found, "/timeline/2"), "");
    assert_eq!(peek(&found, "/changes/0/kind"), r#""move""#);
    assert_eq!(peek(&found, "/changes/0/keys"), "1");
    assert_eq!(peek(&found, "/changes/1"), "");
    assert!(shelf.detail("none").unwrap().is_none());
}

#[test]
fn overview_fails_on_missing_database() {
    let sandbox = Sandbox::new("topics-overview-missing").unwrap();
    let path = sandbox.database();
    let err = Shelf::new(&path, BUSY)
        .overview(&Listing::new("", LIMIT))
        .unwrap_err();
    assert_eq!(err.status(), 503);
}

#[test]
fn overview_fails_on_undecodable_row() {
    let sandbox = seeded("topics-overview-undecodable", BROKEN).unwrap();
    let path = sandbox.database();
    let err = Shelf::new(&path, BUSY)
        .overview(&Listing::new("", LIMIT))
        .unwrap_err();
    assert_eq!(err.status(), 503);
}

#[test]
fn overview_fails_without_tables() {
    let sandbox = Sandbox::new("topics-overview-table").unwrap();
    sandbox.seed("CREATE TABLE other (key TEXT);").unwrap();
    let path = sandbox.database();
    let err = Shelf::new(&path, BUSY)
        .overview(&Listing::new("", LIMIT))
        .unwrap_err();
    assert_eq!(err.status(), 503);
}

#[test]
fn overview_lists_latest_first_with_tallies() {
    let sandbox = seeded("topics-overview", ROWS).unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, BUSY);
    let all = to_value(shelf.overview(&Listing::new("", LIMIT)).unwrap()).unwrap();
    assert_eq!(peek(&all, "/items/0/topic_id"), r#""t2""#);
    assert_eq!(peek(&all, "/items/0/verdict"), r#""done""#);
    assert_eq!(peek(&all, "/items/0/score"), "0.25");
    assert_eq!(peek(&all, "/items/1/topic_id"), r#""t1""#);
    assert_eq!(peek(&all, "/items/1/score"), "0.75");
    assert_eq!(peek(&all, "/items/1/previous"), "0.5");
    assert_eq!(peek(&all, "/items/1/first_at_ms"), "1");
    assert_eq!(peek(&all, "/items/0/previous"), "null");
    assert_eq!(peek(&all, "/items/0/first_at_ms"), "3");
    assert_eq!(peek(&all, "/items/1/sources"), r#"["imap","dingtalk"]"#);
    assert_eq!(
        peek(&all, "/lines"),
        r#"[{"count":1,"reason":"","state":"analysed"},{"count":1,"reason":"notice","state":"skipped"},{"count":1,"reason":"screen","state":"withheld"}]"#
    );
    assert_eq!(
        peek(&all, "/topics"),
        r#"[{"count":1,"reason":"","state":"open"},{"count":1,"reason":"","state":"resolved"}]"#
    );
    let open = to_value(shelf.overview(&Listing::new("open", LIMIT)).unwrap()).unwrap();
    assert_eq!(peek(&open, "/items/0/topic_id"), r#""t1""#);
    assert_eq!(peek(&open, "/items/1"), "");
}

fn peek(value: &Value, pointer: &str) -> String {
    value.pointer(pointer).map_or_default(Value::to_string)
}

fn seeded(name: &str, rows: &str) -> Fallible<Sandbox> {
    let sandbox = Sandbox::new(name)?;
    drop(Archive::open(&sandbox.database(), BUSY)?);
    sandbox.seed(rows)?;
    Ok(sandbox)
}
