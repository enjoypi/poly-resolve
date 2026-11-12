use serde_json::{Error, Value, from_str, json};

use crate::support::{Fixture, local};

const INTERNAL: u64 = 1_783_483_235_000;
const RECEIVED: u64 = 1_783_483_240_000;
const UID: u32 = 42;
const VALIDITY: u32 = 7;
const WIDE: u32 = 1_000;

type Reply = Result<Value, u16>;

#[test]
fn attachments_are_listed_by_type_and_name() {
    let raw = concat!(
        "From: a@x.test\r\nSubject: files\r\nContent-Type: multipart/mixed; boundary=\"b\"\r\n\r\n",
        "--b\r\nContent-Type: text/plain\r\n\r\nsee attached\r\n",
        "--b\r\nContent-Type: application/pdf; name=\"re\x07port.pdf\"\r\nContent-Disposition: attachment; filename=\"re\x07port.pdf\"\r\n\r\nJVBERi0=\r\n",
        "--b\r\nContent-Disposition: attachment\r\n\r\nbare\r\n",
        "--b\r\nContent-Type: image\r\nContent-Disposition: attachment; filename=a\r\n\r\nx\r\n",
        "--b--\r\n",
    );
    let reply = mail(raw.as_bytes(), WIDE).unwrap().unwrap();
    assert_eq!(
        field(&reply, "/envelope/text"),
        "files\nsee attached\n[file application/pdf report.pdf]\n[file  ]\n[file image a]"
    );
}

fn field<'reply>(reply: &'reply Value, pointer: &str) -> &'reply str {
    reply
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or_default()
}

#[test]
fn html_in_legacy_charsets_becomes_text() {
    let mut raw = b"From: =?gbk?B?xOO6ww==?= <B@X.test>\r\nSubject: hi\r\nContent-Type: text/html; charset=gbk\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\n<p>=C4=E3=BA=C3</p>\r\n".to_vec();
    raw.extend_from_slice(b"\r\n");
    let reply = mail(&raw, WIDE).unwrap().unwrap();
    assert_eq!(field(&reply, "/envelope/text"), "hi\n\u{4f60}\u{597d}");
    assert_eq!(field(&reply, "/envelope/sender_name"), "\u{4f60}\u{597d}");
    assert_eq!(field(&reply, "/envelope/sender_id"), "b@x.test");
    assert_eq!(
        field(&reply, "/envelope/conversation_id"),
        "mail:work:thread:uid:7:42"
    );
}

#[test]
fn imap_dates_are_day_precise() {
    let reply = send_to("since", &json!({"at_ms": INTERNAL}).to_string())
        .unwrap()
        .unwrap();
    assert_eq!(reply, json!({"date": "8-Jul-2026"}));
    assert_eq!(send_to("since", "nope").unwrap(), Err(400));
}

fn mail(raw: &[u8], chars: u32) -> Result<Reply, Error> {
    mail_at(raw, chars, " 8-Jul-2026 12:00:35 +0800")
}

fn mail_at(raw: &[u8], chars: u32, internal: &str) -> Result<Reply, Error> {
    let (depth, parts): (u32, u32) = (8, 50);
    let request = json!({
        "account": "work",
        "internal": internal,
        "limits": {"chars": chars, "depth": depth, "parts": parts},
        "raw": raw,
        "received_at_ms": RECEIVED,
        "source": "imap",
        "style": {
            "attachment": "[file {type} {name}]",
            "quote_ends": ["wrote:"],
            "quote_starts": ["-----Original Message-----"],
        },
        "uid": UID,
        "uid_validity": VALIDITY,
    })
    .to_string();
    send(&request)
}

#[test]
fn malformed_requests_are_rejected() {
    assert_eq!(send("nope").unwrap(), Err(400));
    assert_eq!(mail(b"", WIDE).unwrap(), Err(400));
}

#[test]
fn missing_ids_fall_back_to_uid() {
    let raw = b"Subject: x\r\n\r\nbody\r\nOn Mon someone wrote:\r\nold\r\n";
    let reply = mail_at(raw, WIDE, "garbage").unwrap().unwrap();
    assert_eq!(
        reply.pointer("/envelope/occurred_at_ms"),
        Some(&json!(RECEIVED))
    );
    assert_eq!(field(&reply, "/envelope/key"), "imap:work:uid:7:42");
    assert_eq!(field(&reply, "/envelope/sender_id"), "");
    assert_eq!(field(&reply, "/envelope/text"), "x\nbody");
    let answer =
        b"Message-ID: <m2@x.test>\r\nIn-Reply-To: <m1@x.test>\r\nSubject: x\r\n\r\nbody\r\n";
    let replied = mail(answer, WIDE).unwrap().unwrap();
    assert_eq!(
        field(&replied, "/envelope/conversation_id"),
        "mail:work:thread:m1@x.test"
    );
}

#[test]
fn plain_mail_keeps_fresh_text_and_header_summary() {
    let raw = concat!(
        "Message-ID: <m1@x.test>\r\nFrom: \"Alice\" <alice@x.test>\r\nTo: bob@x.test\r\n",
        "Date: Wed, 08 Jul 2026 12:00:35 +0800\r\nSubject: deploy\r\nIn-Reply-To: <m0@x.test>\r\n",
        "References: <a@x.test> <m0@x.test>\r\n\r\n",
        "done\r\n> earlier\r\nnext\r\n-----Original Message-----\r\nold\r\n",
    );
    let reply = mail(raw.as_bytes(), WIDE).unwrap().unwrap();
    assert_eq!(field(&reply, "/envelope/key"), "imap:work:m1@x.test");
    assert_eq!(
        field(&reply, "/envelope/conversation_id"),
        "mail:work:thread:a@x.test"
    );
    assert_eq!(field(&reply, "/envelope/text"), "deploy\ndone\nnext");
    assert_eq!(field(&reply, "/envelope/sender_name"), "Alice");
    assert_eq!(
        reply.pointer("/envelope/occurred_at_ms"),
        Some(&json!(INTERNAL))
    );
    assert_eq!(reply.pointer("/skip"), Some(&Value::Null));
    let summary: Value = from_str(field(&reply, "/envelope/raw")).unwrap();
    assert_eq!(summary.get("in_reply_to"), Some(&json!("m0@x.test")));
    assert_eq!(
        summary.get("references"),
        Some(&json!(["a@x.test", "m0@x.test"]))
    );
    assert_eq!(summary.get("from"), Some(&json!("alice@x.test")));
    assert!(summary.get("date").and_then(Value::as_str).is_some());
    assert!(!field(&reply, "/envelope/raw").contains("bob@x.test"));
    let short = mail(raw.as_bytes(), 3).unwrap().unwrap();
    assert_eq!(field(&short, "/envelope/text"), "dep");
}

fn send(request: &str) -> Result<Reply, Error> {
    send_to("parse_mail", request)
}

fn send_to(action: &str, request: &str) -> Result<Reply, Error> {
    match Fixture::new()?.dispatch(&local("ingest", action, request)) {
        Ok(body) => Ok(Ok(from_str(&body)?)),
        Err(err) => Ok(Err(err.status())),
    }
}

fn tagged(header: &str) -> (String, String) {
    let raw = format!("From: a@x.test\r\n{header}\r\nSubject: s\r\n\r\nbody\r\n");
    let reply = mail(raw.as_bytes(), WIDE)
        .ok()
        .and_then(Result::ok)
        .unwrap_or_default();
    let summary: Value = from_str(field(&reply, "/envelope/raw")).unwrap_or_default();
    (
        field(&reply, "/skip").to_owned(),
        field(&summary, "/bulk").to_owned(),
    )
}

#[test]
fn unauthenticated_mail_is_skipped_and_automatic_mail_is_tagged() {
    let pair = |skip: &str, bulk: &str| (skip.to_owned(), bulk.to_owned());
    assert_eq!(
        tagged("Authentication-Results: mx; dmarc=fail"),
        pair("dmarc", "")
    );
    assert_eq!(
        tagged("Authentication-Results: mx; dmarc=pass"),
        pair("", "")
    );
    assert_eq!(tagged("Auto-Submitted: auto-generated"), pair("", "auto"));
    assert_eq!(tagged("Auto-Submitted: no"), pair("", ""));
    assert_eq!(tagged("List-Id: <news.x.test>"), pair("", "list"));
    assert_eq!(tagged("Precedence: bulk"), pair("", "list"));
    assert_eq!(tagged("Precedence: first-class"), pair("", ""));
}
