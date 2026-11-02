mod failure;
mod fake;
mod target;

use adapters::config::Settings;
use serde_json::{Value, from_value, json};

use self::fake::{Step, fetched, hello, serve};
use crate::support::{Fallible, Sandbox, keys, merge, run_logged};

const BACKFILL: u64 = 86_400_000;
const BATCH: u32 = 1;
const BYTES: u32 = 0x0001_0000;
const CHARS: u32 = 1_000;
const DEPTH: u32 = 8;
const DMARC: &str = "Message-ID: <d@x.test>\r\nFrom: d@x.test\r\nAuthentication-Results: mx; dmarc=fail\r\nSubject: s\r\n\r\nspam\r\n";
const FIRST: &str = "Message-ID: <a@x.test>\r\nFrom: A <a@x.test>\r\nSubject: hi\r\n\r\nbody\r\n";
const IDLE: u64 = 200;
const LINE: u32 = 0x1000;
const LONG: u64 = 2_500;
const OVERLAP: u64 = 1_000;
const PARTS: u32 = 50;
const POLL: u64 = 50;
const SEARCH: u32 = 2;
const SECOND: &str = "Message-ID: <b@x.test>\r\nFrom: b@x.test\r\nSubject: again\r\n\r\nmore\r\n";
const SPAN: u64 = 1_000;
const TIMEOUT: u64 = 1_000;
const TRACED: [&str; 13] = [
    "\"status\":\"skip:dmarc\"",
    "\"status\":\"rejected\"",
    "imap:bye",
    "imap:greeting",
    "\"action\":\"LOGIN\"",
    "\"action\":\"UID FETCH\"",
    "\"params\":\"6\"",
    "\"action\":\"connect\"",
    "\"params\":\"plain+direct\"",
    "\"session_id\":\"work#",
    "\"params\":\"3:9\"",
    "\"status\":\"exists\"",
    "\"transport\":\"imap\"",
];

fn after(step: &'static str, reply: &str) -> Step {
    (step, reply.to_owned())
}

#[tokio::test]
async fn idle_session_fetches_new_mail_and_skips_unwanted() {
    let sandbox = Sandbox::new("imap-idle").unwrap();
    let mut first = hello("IDLE ID");
    first.insert(2, after("ID", "* ID NIL\r\n<tag> OK\r\n"));
    first.extend(vec![
        after("UID SEARCH", "* SEARCH 5 6 7\r\n<tag> OK\r\n"),
        (
            "UID FETCH 6 ",
            format!(
                "{}<tag> OK\r\n",
                fetched(6, FIRST).replacen("Subject", "\u{0}Subject", 1)
            ),
        ),
        (
            "UID FETCH 7 ",
            format!(
                "* 7 FETCH (FLAGS (\\Seen))\r\n{}<tag> OK\r\n",
                fetched(7, DMARC)
            ),
        ),
        after(
            "IDLE",
            "+ idling\r\n\u{0}* OK still here\r\n\u{0}* 9 EXISTS\r\n",
        ),
        after("DONE", "<tag> OK idle done\r\n"),
        (
            "UID FETCH 8:* ",
            format!(
                "{}{}{}<tag> OK\r\n",
                fetched(7, FIRST),
                fetched(9, SECOND),
                fetched(10, "")
            ),
        ),
        after("IDLE", "+ idling\r\n\u{0}* BYE closing\r\n"),
    ]);
    let mut second = hello("");
    second.push(after("UID SEARCH", "* SEARCH\r\n<tag> OK\r\n"));
    let third = vec![after("", "* BYE busy\r\n")];
    let fake = serve(vec![first, second, third]).unwrap();
    let config = settings(&sandbox, &fake.url(), &json!({})).unwrap();
    let (code, logs) = run_logged(&config, LONG).await.unwrap();
    assert_eq!(code, 0);
    assert_eq!(
        keys(&sandbox.database()).unwrap(),
        ["imap:work:a@x.test", "imap:work:b@x.test"]
    );
    let absent: Vec<&str> = TRACED
        .into_iter()
        .filter(|part| !logs.contains(part))
        .collect();
    assert_eq!(absent, Vec::<&str>::new());
    assert!(!logs.contains("s3cret"));
    assert!(!logs.contains("me@x.test"));
    assert!(!logs.contains("a@x.test"));
    let commands = fake.finish();
    assert!(commands.contains(&"LOGIN \"me@x.test\" \"s3cret\\\"pw\"".to_owned()));
    assert!(commands.contains(&"UID FETCH 6 (UID INTERNALDATE BODY.PEEK[]<0.65536>)".to_owned()));
}

#[tokio::test]
async fn polling_refresh_and_refusals_back_off() {
    let sandbox = Sandbox::new("imap-poll").unwrap();
    let mut polling = hello("");
    polling.extend(vec![
        after("UID SEARCH", "* SEARCH\r\n<tag> OK\r\n"),
        after("UID FETCH 8:*", "<tag> OK\r\n"),
    ]);
    let mut refused = hello("IDLE");
    refused.truncate(2);
    refused.push(after("LOGIN", "<tag> NO [AUTHENTICATIONFAILED] bad\r\n"));
    let mut refreshed = hello("IDLE");
    refreshed.extend(vec![
        after("UID SEARCH", "* SEARCH\r\n<tag> OK\r\n"),
        after("IDLE", "+ idling\r\n"),
        after("DONE", "<tag> OK\r\n"),
        after("UID FETCH 8:*", "<tag> OK\r\n"),
        after("IDLE", "<tag> BAD no\r\n"),
    ]);
    let mut busy = hello("");
    busy.truncate(2);
    busy.push(after("LOGIN", "<tag> NO [UNAVAILABLE] later\r\n"));
    let mut named = hello("ID");
    named.insert(2, after("ID", "<tag> NO nope\r\n"));
    let fake = serve(vec![polling, refused, refreshed, busy, named]).unwrap();
    let config = settings(&sandbox, &fake.url(), &json!({"idle_ms": IDLE})).unwrap();
    let (code, logs) = run_logged(&config, LONG).await.unwrap();
    assert_eq!(code, 0);
    for part in [
        "imap:eof",
        "imap login refused",
        "imap:idle",
        "imap:no",
        "\"status\":\"refresh\"",
        "\"status\":\"imap:auth\"",
    ] {
        assert!(logs.contains(part), "{part}");
    }
    drop(fake.finish());
}

pub(crate) fn settings(sandbox: &Sandbox, url: &str, patch: &Value) -> Fallible<Settings> {
    whole(sandbox, &source(url, patch), &json!({}))
}

pub(crate) fn source(url: &str, patch: &Value) -> Value {
    let mut source = json!({
        "account": "work",
        "idle_ms": SPAN,
        "initial_backfill_ms": BACKFILL,
        "limits": {
            "chars": CHARS,
            "depth": DEPTH,
            "fetch_batch": BATCH,
            "line_bytes": LINE,
            "message_bytes": BYTES,
            "parts": PARTS,
            "search_max": SEARCH,
        },
        "mailbox": "INBOX",
        "password": "${POLY_RESOLVE_TEST_IMAP_PASSWORD}",
        "poll_ms": POLL,
        "reconcile_overlap_ms": OVERLAP,
        "self_sender_ids": ["${POLY_RESOLVE_TEST_IMAP_USER}"],
        "style": {"attachment": "[file {type} {name}]", "quote_ends": [], "quote_starts": []},
        "timeout_ms": TIMEOUT,
        "url": url,
        "user": "${POLY_RESOLVE_TEST_IMAP_USER}",
    });
    merge(&mut source, patch);
    source
}

pub(crate) fn whole(sandbox: &Sandbox, imap: &Value, shared: &Value) -> Fallible<Settings> {
    let mut config = sandbox.shared(&json!([{ "imap": imap }]));
    merge(&mut config, shared);
    Ok(from_value(config)?)
}
