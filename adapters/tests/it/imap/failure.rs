use core::time::Duration;

use adapters::store::Archive;
use rusqlite::Connection;
use serde_json::json;

use super::fake::{Script, fetched, hello, serve};
use super::{FIRST, after, settings, source, whole};
use crate::support::{FAILURE, REFUSE_INSERT, Sandbox, run_logged};

const BRIEF: u64 = 50;
const CLIMB: u64 = 4_000;
const FAILURES: usize = 5;
const GAP: Duration = Duration::from_millis(600);
const IDLE: u64 = 100;
const NARROW: u32 = 64;
const QUICK: u64 = 200;
const RESET: u64 = 3_000;
const SHORT: u64 = 1_500;
const STOP_IDLE: u64 = 60_000;
const WATERMARK: &str = "CREATE TABLE messages (key TEXT PRIMARY KEY, source TEXT, account TEXT, occurred_at_ms TEXT); INSERT INTO messages VALUES ('k', 'imap', 'work', 'abc');";

#[tokio::test]
async fn busy_store_backs_off() {
    let sandbox = Sandbox::new("imap-busy").unwrap();
    drop(Archive::open(&sandbox.database(), BRIEF).unwrap());
    let holder = Connection::open(sandbox.database()).unwrap();
    holder.execute_batch("BEGIN IMMEDIATE").unwrap();
    let mut script = hello("");
    script.extend(vec![
        after("UID SEARCH", "* SEARCH 6\r\n<tag> OK\r\n"),
        ("UID FETCH 6 ", format!("{}<tag> OK\r\n", fetched(6, FIRST))),
    ]);
    let fake = serve(vec![script]).unwrap();
    let busy = json!({"busy_timeout_ms": BRIEF});
    let config = whole(&sandbox, &source(&fake.url(), &json!({})), &busy).unwrap();
    let (code, logs) = run_logged(&config, SHORT).await.unwrap();
    holder.execute_batch("ROLLBACK").unwrap();
    assert_eq!(code, 0);
    assert!(logs.contains("store busy"));
    drop(fake.finish());
}

#[tokio::test]
async fn finished_round_resets_backoff() {
    let sandbox = Sandbox::new("imap-round-reset").unwrap();
    let mut sessions = vec![vec![after("", "* NO busy\r\n")]; FAILURES];
    let mut idle = hello("IDLE");
    idle.extend(vec![
        after("UID SEARCH", "* SEARCH\r\n<tag> OK\r\n"),
        after("IDLE", "+ idling\r\n"),
        after("DONE", "<tag> OK\r\n"),
        after("UID FETCH 8:*", "<tag> OK\r\n"),
    ]);
    sessions.push(idle);
    sessions.push(vec![after("", "* NO busy\r\n")]);
    let fake = serve(sessions).unwrap();
    let shared = json!({"backoff_initial_ms": BRIEF, "backoff_max_ms": CLIMB});
    let imap = source(&fake.url(), &json!({"idle_ms": IDLE}));
    let config = whole(&sandbox, &imap, &shared).unwrap();
    let (_code, logs) = run_logged(&config, RESET).await.unwrap();
    let spans = fake.timed(FAILURES + 2);
    let &[.., before, round, after] = spans.as_slice() else {
        panic!("{spans:?}");
    };
    assert!(logs.contains("\"rounds\":1"), "{logs}");
    let climbed = round.0.duration_since(before.1);
    let reset = after.0.duration_since(round.1);
    assert!(climbed >= GAP, "{climbed:?}");
    assert!(reset < GAP, "{reset:?}");
    drop(fake.finish());
}

fn held(steps: &[(&'static str, &str)]) -> Script {
    let mut script: Script = steps
        .iter()
        .map(|&(step, reply)| (step, reply.to_owned()))
        .collect();
    script.push(("NEVER", String::new()));
    script
}

#[tokio::test]
async fn protocol_faults_end_the_session() {
    let sandbox = Sandbox::new("imap-faults").unwrap();
    let mut huge = hello("");
    huge.extend(vec![
        after("UID SEARCH", "* SEARCH 6\r\n<tag> OK\r\n"),
        (
            "UID FETCH 6 ",
            format!("{}<tag> OK\r\n", fetched(6, &"x".repeat(200))),
        ),
    ]);
    let mut wide = hello("");
    wide.truncate(3);
    wide.push(after(
        "SELECT",
        &format!("* OK {}\u{0}\r\n", "x".repeat(200)),
    ));
    let mut bye = hello("");
    bye.push(after("UID SEARCH", "* BYE bye\r\n"));
    let mut cut = hello("");
    cut.extend(vec![
        after("UID SEARCH", "* SEARCH 6\r\n<tag> OK\r\n"),
        after("UID FETCH 6 ", "* 6 FETCH (UID 6 BODY[] {50}\r\nshort"),
    ]);
    let mut odd = hello("");
    odd.truncate(3);
    odd.push(after("SELECT", "* OK odd}\r\n<tag> OK\r\n"));
    let fake = serve(vec![
        huge,
        wide,
        bye,
        cut,
        odd,
        vec![after("", "")],
        held(&[("", "")]),
        held(&[("", "* OK ready\r\n"), ("CAPABILITY", "")]),
    ])
    .unwrap();
    let patch =
        json!({"limits": {"line_bytes": NARROW, "message_bytes": NARROW}, "timeout_ms": QUICK});
    let config = settings(&sandbox, &fake.url(), &patch).unwrap();
    let (code, logs) = run_logged(&config, SHORT).await.unwrap();
    assert_eq!(code, 0);
    for part in [
        "imap:literal",
        "imap:line",
        "imap:bye",
        "timeout:greeting",
        "timeout:read",
    ] {
        assert!(logs.contains(part), "{part}");
    }
    drop(fake.finish());
}

#[tokio::test]
async fn silent_idle_times_out() {
    let sandbox = Sandbox::new("imap-idle-silent").unwrap();
    let mut start = hello("IDLE");
    start.push(after("UID SEARCH", "* SEARCH\r\n<tag> OK\r\n"));
    start.push(after("IDLE", ""));
    start.push(("NEVER", String::new()));
    let mut done = hello("IDLE");
    done.push(after("UID SEARCH", "* SEARCH\r\n<tag> OK\r\n"));
    done.push(after("IDLE", "+ idling\r\n"));
    done.push(after("DONE", ""));
    done.push(("NEVER", String::new()));
    let mut dropped = hello("IDLE");
    dropped.push(after("UID SEARCH", "* SEARCH\r\n<tag> OK\r\n"));
    dropped.push(after("IDLE", ""));
    let mut quiet = hello("IDLE");
    quiet.push(after("UID SEARCH", "* SEARCH\r\n<tag> OK\r\n"));
    quiet.push(after("IDLE", "+ idling\r\n"));
    let fake = serve(vec![start, done, dropped, quiet]).unwrap();
    let patch = json!({"idle_ms": IDLE, "timeout_ms": QUICK});
    let config = settings(&sandbox, &fake.url(), &patch).unwrap();
    let (_code, logs) = run_logged(&config, SHORT).await.unwrap();
    assert_eq!(
        logs.matches("\"reason\":\"timeout:idle\"").count(),
        2,
        "{logs}"
    );
    drop(fake.finish());
}

#[tokio::test]
async fn stop_interrupts_a_waiting_session() {
    let sandbox = Sandbox::new("imap-stop").unwrap();
    let mut script = hello("IDLE");
    script.push(after("UID SEARCH", "* SEARCH\r\n<tag> OK\r\n"));
    script.push(after("IDLE", "+ idling\r\n"));
    script.push(("NEVER", String::new()));
    let fake = serve(vec![script]).unwrap();
    let config = settings(&sandbox, &fake.url(), &json!({"idle_ms": STOP_IDLE})).unwrap();
    let (code, logs) = run_logged(&config, SHORT).await.unwrap();
    assert_eq!(code, 0);
    assert!(!logs.contains("imap session ended"));
    drop(fake.finish());
}

#[tokio::test]
async fn store_mark_failure_stops_the_source() {
    let sandbox = Sandbox::new("imap-store-mark").unwrap();
    sandbox.seed(WATERMARK).unwrap();
    let fake = serve(vec![hello("")]).unwrap();
    let config = settings(&sandbox, &fake.url(), &json!({})).unwrap();
    let (code, _logs) = run_logged(&config, SHORT).await.unwrap();
    assert_eq!(code, FAILURE);
    drop(fake.finish());
}

#[tokio::test]
async fn store_open_failure_stops_the_source() {
    let sandbox = Sandbox::new("imap-store-missing").unwrap();
    let absent = json!({"store_path": "/nonexistent/x.db"});
    let config = whole(&sandbox, &source("imap://127.0.0.1:1", &json!({})), &absent).unwrap();
    let (code, _logs) = run_logged(&config, SHORT).await.unwrap();
    assert_eq!(code, FAILURE);
}

#[tokio::test]
async fn store_write_failure_stops_the_source() {
    let sandbox = Sandbox::new("imap-store-refuse").unwrap();
    drop(Archive::open(&sandbox.database(), BRIEF).unwrap());
    sandbox.seed(REFUSE_INSERT).unwrap();
    let mut script = hello("");
    script.extend(vec![
        after("UID SEARCH", "* SEARCH 6\r\n<tag> OK\r\n"),
        ("UID FETCH 6 ", format!("{}<tag> OK\r\n", fetched(6, FIRST))),
    ]);
    let fake = serve(vec![script]).unwrap();
    let config = settings(&sandbox, &fake.url(), &json!({})).unwrap();
    let (code, _logs) = run_logged(&config, SHORT).await.unwrap();
    assert_eq!(code, FAILURE);
    drop(fake.finish());
}

#[tokio::test]
async fn unusable_mailbox_name_ends_the_session() {
    let sandbox = Sandbox::new("imap-mailbox").unwrap();
    let fake = serve(vec![hello("")]).unwrap();
    let config = settings(&sandbox, &fake.url(), &json!({"mailbox": "\u{6536}"})).unwrap();
    let (_code, logs) = run_logged(&config, SHORT).await.unwrap();
    assert!(logs.contains("imap:credential"));
    drop(fake.finish());
}
