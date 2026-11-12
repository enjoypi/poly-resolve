use adapters::store::{Archive, Shelf};
use domain::outbound::{Item, Listing, Outbox as _};
use serde_json::{Value, to_value};

use crate::support::{BUSY, Sandbox};

const LIMIT: u32 = 2;
const ORIGIN: &str = "INSERT INTO messages (key, source, account, conversation_id, sender_id, sender_name, text, occurred_at_ms, received_at_ms, raw) VALUES ('b', 'imap', 'hf', 'c1', 's', 'alice', 'text b', 1, 1, '');";
const ROWS: &str = "INSERT INTO lines (key, conversation_id, occurred_at_ms, sender, text, state, reason, reviewer, flag, hints, rules_rev, batch_id, created_at_ms, updated_at_ms, role) VALUES ('a', 'c1', 3, 'alice', 'text a', 'approved', '', 'auto', '', '[]', 0, 1, 3, 3, 'own'), ('b', 'c1', 1, 'alice', 'text b', 'approved', '', 'auto', '', '[]', 0, 1, 1, 1, ''), ('c', 'c1', 2, 'alice', 'text c', 'approved', '', 'auto', '', '[]', 0, 1, 2, 2, ''), ('d', 'c1', 4, '', '', 'withheld', 'sealed', '', '', '[]', 0, 0, 4, 4, '');";

fn fields(items: &[Item], name: &str) -> Vec<String> {
    items
        .iter()
        .filter_map(|item| {
            let value = to_value(item).ok()?;
            value.get(name).and_then(Value::as_str).map(str::to_owned)
        })
        .collect()
}

#[test]
fn list_fails_on_missing_database() {
    let sandbox = Sandbox::new("outbound-list-missing").unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, BUSY);
    let err = shelf.list(&Listing::new("approved", LIMIT)).unwrap_err();
    assert_eq!(err.status(), 503);
    assert!(!sandbox.database().exists());
}

#[test]
fn list_fails_on_undecodable_row() {
    let sandbox = Sandbox::new("outbound-list-undecodable").unwrap();
    sandbox
        .seed("CREATE TABLE lines (key TEXT, conversation_id TEXT, occurred_at_ms INTEGER, sender TEXT, text TEXT, state TEXT, reason TEXT, role TEXT, flag TEXT, batch_id TEXT); INSERT INTO lines VALUES ('k', 'c', 1, 's', 't', 'approved', '', '', '', 'abc'); CREATE TABLE messages (key TEXT, source TEXT, account TEXT);")
        .unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, BUSY);
    let err = shelf.list(&Listing::new("approved", LIMIT)).unwrap_err();
    assert_eq!(err.status(), 503);
}

#[test]
fn list_fails_without_lines_table() {
    let sandbox = Sandbox::new("outbound-list-table").unwrap();
    sandbox.seed("CREATE TABLE other (key TEXT);").unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, BUSY);
    let err = shelf.list(&Listing::new("approved", LIMIT)).unwrap_err();
    assert_eq!(err.status(), 503);
}

#[test]
fn list_returns_newest_first_within_limit() {
    let sandbox = Sandbox::new("outbound-list").unwrap();
    drop(Archive::open(&sandbox.database(), BUSY).unwrap());
    sandbox.seed(ROWS).unwrap();
    sandbox.seed(ORIGIN).unwrap();
    let path = sandbox.database();
    let shelf = Shelf::new(&path, BUSY);
    let sent = shelf.list(&Listing::new("approved", LIMIT)).unwrap();
    assert_eq!(fields(&sent, "key"), ["a", "c"]);
    assert_eq!(fields(&sent, "role"), ["own", ""]);
    let all = shelf.list(&Listing::new("approved", LIMIT + 1)).unwrap();
    assert_eq!(fields(&all, "source"), ["", "", "imap"]);
    assert_eq!(fields(&all, "account"), ["", "", "hf"]);
    let held = shelf.list(&Listing::new("withheld", LIMIT)).unwrap();
    assert_eq!(fields(&held, "key"), ["d"]);
    assert_eq!(fields(&held, "reason"), ["sealed"]);
}
