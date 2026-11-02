use adapters::store::{Archive, Shelf};
use domain::review::{Docket as _, Draw};
use rusqlite::{Connection, Error};
use tracing::dispatcher::with_default;

use crate::capture::Capture;
use crate::support::{BUSY, Fallible, Sandbox};

const LINES: &str = "SELECT group_concat(item, ',') FROM (SELECT key || '|' || state || '|' || batch_id AS item FROM lines ORDER BY key)";
const ROWS: &str = "INSERT INTO lines (key, conversation_id, occurred_at_ms, sender, text, state, reason, reviewer, flag, hints, rules_rev, batch_id, created_at_ms, updated_at_ms) VALUES ('a', 'c1', 1, 's', 't', 'queued', '', '', '', '[]', 0, 0, 1, 1), ('b', 'c1', 3, 's', 't', 'queued', '', '', '', '[]', 0, 0, 3, 3), ('c', 'c1', 2, 's', 't', 'queued', '', '', '', '[]', 0, 0, 2, 2), ('d', 'c1', 4, 's', 't', 'skipped', 'self', '', '', '[]', 0, 0, 4, 4), ('e', 'c1', 0, 's', 't', 'sent', '', '', '', '[]', 0, 3, 0, 0);";
const TWO: u32 = 2;

#[test]
fn draw_fails_on_broken_storage() {
    let missing = Sandbox::new("review-draw-missing").unwrap();
    let capture = Capture::default();
    let absent = with_default(&capture.dispatch(), || drawn(&missing, Draw::reviewed(TWO)));
    assert_eq!(absent.unwrap_err().to_string(), "status 503");
    assert!(capture.text().contains("review draw failed"));
    let bare = Sandbox::new("review-draw-table").unwrap();
    bare.seed("CREATE TABLE other (key TEXT);").unwrap();
    let tableless = drawn(&bare, Draw::reviewed(TWO)).unwrap_err();
    assert_eq!(tableless.to_string(), "status 503");
    let refused = seeded("review-draw-refused").unwrap();
    refused
        .seed("CREATE TRIGGER refuse BEFORE UPDATE ON lines BEGIN SELECT RAISE(ABORT, 'refused'); END;")
        .unwrap();
    let blocked = drawn(&refused, Draw::reviewed(TWO)).unwrap_err();
    assert_eq!(blocked.to_string(), "status 503");
    assert_eq!(
        lines(&refused).unwrap(),
        "a|queued|0,b|queued|0,c|queued|0,d|skipped|0,e|sent|3"
    );
}

#[test]
fn draw_takes_the_newest_queued_lines_once_per_review() {
    let sandbox = seeded("review-draw").unwrap();
    assert_eq!(drawn(&sandbox, Draw::reviewed(TWO)).unwrap(), "4|2");
    assert_eq!(
        lines(&sandbox).unwrap(),
        "a|queued|0,b|pending|4,c|pending|4,d|skipped|0,e|sent|3"
    );
    assert_eq!(drawn(&sandbox, Draw::reviewed(TWO)).unwrap(), "busy");
    sandbox
        .seed("UPDATE lines SET state = 'approved' WHERE batch_id = 4;")
        .unwrap();
    assert_eq!(drawn(&sandbox, Draw::screened(TWO)).unwrap(), "5|1");
    assert!(lines(&sandbox).unwrap().starts_with("a|screening|5,"));
    sandbox
        .seed("UPDATE lines SET state = 'leaked' WHERE batch_id = 5;")
        .unwrap();
    assert_eq!(drawn(&sandbox, Draw::reviewed(TWO)).unwrap(), "busy");
    sandbox
        .seed("UPDATE lines SET state = 'rejected' WHERE batch_id = 5;")
        .unwrap();
    assert_eq!(drawn(&sandbox, Draw::reviewed(TWO)).unwrap(), "0|0");
}

fn drawn(sandbox: &Sandbox, policy: Draw) -> Fallible<String> {
    let path = sandbox.database();
    let shelf = Shelf::new(&path, BUSY);
    let pick = policy.pick().ok_or("draw is off")?;
    let found = shelf
        .draw(pick)
        .map_err(|err| format!("status {}", err.status()))?;
    Ok(found.map_or_else(
        || "busy".to_owned(),
        |done| format!("{}|{}", done.batch_id(), done.lines()),
    ))
}

fn lines(sandbox: &Sandbox) -> Fallible<String> {
    let connection = Connection::open(sandbox.database())?;
    let text: String = connection
        .query_row_and_then(LINES, [], |row| row.get(0).map_err(Box::new))
        .map_err(|err: Box<Error>| err)?;
    Ok(text)
}

fn seeded(name: &str) -> Fallible<Sandbox> {
    let sandbox = Sandbox::new(name)?;
    drop(Archive::open(&sandbox.database(), BUSY)?);
    sandbox.seed(ROWS)?;
    Ok(sandbox)
}
