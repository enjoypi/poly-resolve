use adapters::store::{Archive, Shelf};
use domain::review::{Docket as _, Item, Listing, Rule, Ruling};
use rusqlite::{Connection, Error};
use serde_json::{Value, to_value};

use crate::support::{Fallible, Sandbox};

const LIMIT: u32 = 2;
const ROWS: &str = "INSERT INTO lines (key, conversation_id, occurred_at_ms, sender, text, state, reason, reviewer, flag, hints, rules_rev, batch_id, created_at_ms, updated_at_ms) VALUES ('a', 'c1', 3, 'alice', 'text a secret-abc', 'pending', '', '', '', '[]', 0, 1, 3, 3), ('b', 'c1', 1, 'alice', 'text b', 'pending', '', '', '', '[]', 0, 1, 1, 1), ('c', 'c1', 2, 'alice', 'text c', 'pending', '', '', '', '[]', 0, 1, 2, 2), ('d', 'c1', 4, 'bob', 'text d', 'approved', '', 'bob', '', '[]', 0, 1, 4, 5), ('e', 'c1', 5, 'bob', 'text e', 'queued', '', '', '', '[]', 0, 0, 5, 5);";

fn field(sandbox: &Sandbox, key: &str, column: &str) -> Fallible<String> {
    let connection = Connection::open(sandbox.database())?;
    let sql = format!("SELECT {column} FROM lines WHERE key = ?1");
    let text: String = connection
        .query_row_and_then(&sql, [key], |row| row.get(0).map_err(Box::new))
        .map_err(|err: Box<Error>| err)?;
    Ok(text)
}

fn keys(items: &[Item]) -> Vec<String> {
    items
        .iter()
        .filter_map(|item| {
            let value = to_value(item).ok()?;
            value.get("key").and_then(Value::as_str).map(str::to_owned)
        })
        .collect()
}

#[test]
fn list_fails_on_missing_database() {
    let sandbox = Sandbox::new("review-list-missing").unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, "dingtalk", "acct");
    let err = shelf.list(&Listing::new("pending", LIMIT)).unwrap_err();
    assert_eq!(err.status(), 503);
    assert!(!sandbox.database().exists());
}

#[test]
fn list_fails_on_undecodable_row() {
    let sandbox = Sandbox::new("review-list-undecodable").unwrap();
    sandbox
        .seed("CREATE TABLE lines (key TEXT, conversation_id TEXT, occurred_at_ms INTEGER, sender TEXT, text TEXT, state TEXT, reason TEXT, reviewer TEXT, flag TEXT, batch_id TEXT); INSERT INTO lines VALUES ('k', 'c', 1, 's', 't', 'pending', '', '', '', 'abc');")
        .unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, "dingtalk", "acct");
    let err = shelf.list(&Listing::new("pending", LIMIT)).unwrap_err();
    assert_eq!(err.status(), 503);
}

#[test]
fn list_fails_without_lines_table() {
    let sandbox = Sandbox::new("review-list-table").unwrap();
    sandbox.seed("CREATE TABLE other (key TEXT);").unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, "dingtalk", "acct");
    let err = shelf.list(&Listing::new("pending", LIMIT)).unwrap_err();
    assert_eq!(err.status(), 503);
}

#[test]
fn list_returns_oldest_first_within_limit() {
    let sandbox = Sandbox::new("review-list").unwrap();
    drop(Archive::open(&sandbox.database()).unwrap());
    sandbox.seed(ROWS).unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, "dingtalk", "acct");
    let pending = shelf.list(&Listing::new("pending", LIMIT)).unwrap();
    assert_eq!(keys(&pending), ["b", "c"]);
    let approved = shelf.list(&Listing::new("approved", LIMIT)).unwrap();
    assert_eq!(keys(&approved), ["d"]);
    assert!(
        shelf
            .list(&Listing::new("queued", LIMIT))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn settle_fails_on_missing_database() {
    let sandbox = Sandbox::new("review-settle-missing").unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, "dingtalk", "acct");
    let err = shelf.settle(&Ruling::approve("a")).unwrap_err();
    assert_eq!(err.status(), 503);
    assert!(!sandbox.database().exists());
}

#[test]
fn settle_fails_on_unreadable_state() {
    let sandbox = Sandbox::new("review-settle-state").unwrap();
    sandbox
        .seed("CREATE TABLE lines (key TEXT, state INTEGER, reason TEXT, reviewer TEXT, updated_at_ms INTEGER); INSERT INTO lines VALUES ('k', 5, '', '', 0);")
        .unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, "dingtalk", "acct");
    let err = shelf.settle(&Ruling::approve("k")).unwrap_err();
    assert_eq!(err.status(), 503);
}

#[test]
fn settle_fails_without_lines_table() {
    let sandbox = Sandbox::new("review-settle-table").unwrap();
    sandbox.seed("CREATE TABLE other (key TEXT);").unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, "dingtalk", "acct");
    let err = shelf.settle(&Ruling::approve("a")).unwrap_err();
    assert_eq!(err.status(), 503);
}

#[test]
fn settle_learns_a_rule_only_from_pending_lines() {
    let sandbox = Sandbox::new("review-settle-learn").unwrap();
    drop(Archive::open(&sandbox.database()).unwrap());
    sandbox.seed(ROWS).unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, "dingtalk", "acct");
    let rule = Rule::new(r"secret\-abc".to_owned(), "[x]".to_owned());
    let leaked = shelf
        .settle(&Ruling::reject("a", "leak").learned(&rule))
        .unwrap();
    assert_eq!(leaked.as_deref(), Some("leaked"));
    assert_eq!(field(&sandbox, "a", "state").unwrap(), "leaked");
    assert_eq!(field(&sandbox, "a", "reason").unwrap(), "leak");
    let again = shelf
        .settle(&Ruling::reject("d", "leak").learned(&rule))
        .unwrap();
    assert_eq!(again.as_deref(), Some("approved"));
    let connection = Connection::open(sandbox.database()).unwrap();
    let count: Result<i64, Box<Error>> =
        connection.query_row_and_then("SELECT COUNT(*) FROM learned_rules", [], |row| {
            row.get(0).map_err(Box::new)
        });
    assert_eq!(count.unwrap(), 1);
}

#[test]
fn settle_rules_pending_items_once() {
    let sandbox = Sandbox::new("review-settle").unwrap();
    drop(Archive::open(&sandbox.database()).unwrap());
    sandbox.seed(ROWS).unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, "dingtalk", "acct");
    let approved = shelf.settle(&Ruling::approve("a")).unwrap();
    assert_eq!(approved.as_deref(), Some("approved"));
    let again = shelf.settle(&Ruling::reject("a", "noise")).unwrap();
    assert_eq!(again.as_deref(), Some("approved"));
    assert_eq!(field(&sandbox, "a", "reason").unwrap(), "");
    let rejected = shelf.settle(&Ruling::reject("b", "noise")).unwrap();
    assert_eq!(rejected.as_deref(), Some("rejected"));
    assert_eq!(field(&sandbox, "b", "reason").unwrap(), "noise");
    assert_eq!(shelf.settle(&Ruling::approve("nope")).unwrap(), None);
}

#[test]
fn settle_with_a_rule_fails_on_missing_tables() {
    let sandbox = Sandbox::new("review-learn-missing").unwrap();
    sandbox.seed("CREATE TABLE other (key TEXT);").unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, "dingtalk", "acct");
    let rule = Rule::new("x".to_owned(), "[x]".to_owned());
    let err = shelf
        .settle(&Ruling::reject("a", "leak").learned(&rule))
        .unwrap_err();
    assert_eq!(err.status(), 503);
}

#[test]
fn source_fails_on_missing_database_and_bad_text() {
    let sandbox = Sandbox::new("review-source-missing").unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, "dingtalk", "acct");
    assert_eq!(shelf.source("a").unwrap_err().status(), 503);
    sandbox
        .seed("CREATE TABLE lines (key TEXT, text INTEGER); INSERT INTO lines VALUES ('k', 5);")
        .unwrap();
    assert_eq!(shelf.source("k").unwrap_err().status(), 503);
    let bare = Sandbox::new("review-source-table").unwrap();
    bare.seed("CREATE TABLE other (key TEXT);").unwrap();
    let bare_path = bare.database();
    let bare_shelf = Shelf::new(&bare_path, "dingtalk", "acct");
    assert_eq!(bare_shelf.source("k").unwrap_err().status(), 503);
}

#[test]
fn source_returns_the_line_text() {
    let sandbox = Sandbox::new("review-source").unwrap();
    drop(Archive::open(&sandbox.database()).unwrap());
    sandbox.seed(ROWS).unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, "dingtalk", "acct");
    assert_eq!(
        shelf.source("a").unwrap().as_deref(),
        Some("text a secret-abc")
    );
    assert_eq!(shelf.source("nope").unwrap(), None);
}
