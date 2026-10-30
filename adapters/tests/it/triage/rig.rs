use std::env;
use std::io::ErrorKind;

use adapters::store::{Archive, Shelf};
use domain::review::{Docket as _, Ruling};
use rusqlite::{Connection, Error};
use serde_json::{Value, from_str, json};

use crate::support::{BUSY, Fallible, NOON, Sandbox, triage_logged};

pub(super) const ALERTS: &str = "SELECT key || '|' || state || '|' || attempts || '|' || COALESCE(analysis, '') || '|' || reason AS item FROM alerts ORDER BY key";
pub(super) const BATCHES: &str = "SELECT id || '|' || state || '|' || attempts || '|' || reason AS item FROM batches ORDER BY id";
pub(super) const KEY: &str = "dingtalk:acct:m1";
pub(super) const LATER: u64 = 60_000;
pub(super) const LINES: &str = "SELECT key || '|' || state || '|' || reason || '|' || batch_id AS item FROM lines ORDER BY key";
pub(super) const NOTHING: [&str; 0] = [];
pub(super) const PLACEMENTS: &str = "SELECT key || '|' || topic_id || '|' || COALESCE(probability, '') AS item FROM placements ORDER BY key";
pub(super) const QUICK: u64 = 2_500;
pub(super) const SECOND: &str = "dingtalk:acct:m2";
pub(super) const SHORT: u64 = 4_000;
pub(super) const THIRD: &str = "dingtalk:acct:m3";
pub(super) const TOPICS: &str =
    "SELECT id || '|' || label || '|' || thread_id AS item FROM topics ORDER BY id";

pub(super) type Seed<'seed> = (&'seed str, &'seed str, &'seed str, &'seed str);

pub(super) fn calls(sandbox: &Sandbox, role: &str) -> Fallible<usize> {
    match sandbox.read(&format!("{role}-calls.txt")) {
        Ok(text) => Ok(text.lines().count()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(0),
        Err(err) => Err(err.into()),
    }
}

pub(super) fn decided(sandbox: &Sandbox) -> Fallible<Value> {
    Ok(from_str(&sandbox.read("decide-stdin.txt")?)?)
}

pub(super) fn home() -> Fallible<String> {
    Ok(env::var("HOME")?)
}

pub(super) fn input_of(body: &Value) -> Fallible<Value> {
    let text = body
        .get("input")
        .and_then(Value::as_str)
        .ok_or("input missing")?;
    Ok(from_str(text)?)
}

pub(super) async fn logged(sandbox: &Sandbox, patch: &Value, millis: u64) -> Fallible<String> {
    let ingest = sandbox.config(&json!({}))?;
    let triage = sandbox.triage(patch)?;
    triage_logged(&ingest, &triage, millis).await
}

pub(super) const fn one() -> [Seed<'static>; 1] {
    [(KEY, "u1", "c1", "server down")]
}

pub(super) fn path() -> Fallible<String> {
    Ok(env::var("PATH")?)
}

pub(super) fn questions_of(body: &Value) -> Vec<String> {
    body.get("questions")
        .and_then(Value::as_array)
        .map_or_default(|all| {
            all.iter()
                .filter_map(|question| question.get("name").and_then(Value::as_str))
                .map(str::to_owned)
                .collect()
        })
}

pub(super) fn rule(sandbox: &Sandbox, ruling: &Ruling<'_>) -> Fallible<Option<String>> {
    let path = sandbox.database();
    let shelf = Shelf::new(&path, BUSY);
    Ok(shelf
        .settle(ruling)
        .map_err(|err| format!("status {}", err.status()))?)
}

pub(super) fn seed(sandbox: &Sandbox, rows: &[Seed<'_>]) -> Fallible<()> {
    seed_from(sandbox, rows, 0)
}

pub(super) fn seed_from(sandbox: &Sandbox, rows: &[Seed<'_>], shift: u64) -> Fallible<()> {
    let stamped = rows
        .iter()
        .enumerate()
        .map(|(index, &row)| {
            let stamp = NOON
                .saturating_add(shift)
                .saturating_add(u64::try_from(index)?);
            Ok((row, stamp))
        })
        .collect::<Fallible<Vec<_>>>()?;
    seed_stamped(sandbox, &stamped)
}

pub(super) fn seed_stamped(sandbox: &Sandbox, rows: &[(Seed<'_>, u64)]) -> Fallible<()> {
    let archive = Archive::open(&sandbox.database(), BUSY)?;
    for &((key, sender, conversation, text), stamp) in rows {
        let envelope = json!({
            "account": "acct",
            "conversation_id": conversation,
            "key": key,
            "occurred_at_ms": stamp,
            "raw": "{}",
            "received_at_ms": stamp,
            "sender_id": sender,
            "sender_name": "name",
            "source": "dingtalk",
            "text": text,
        });
        let _: usize = archive.save_event(&envelope.to_string())?;
    }
    Ok(())
}

pub(super) async fn start(sandbox: &Sandbox, patch: &Value, millis: u64) -> Fallible<()> {
    drop(logged(sandbox, patch, millis).await?);
    Ok(())
}

pub(super) fn table(sandbox: &Sandbox, sql: &str) -> Fallible<Vec<String>> {
    let connection = Connection::open(sandbox.database())?;
    let wrapped = format!("SELECT json_group_array(item) FROM ({sql})");
    let json: String = connection
        .query_row_and_then(&wrapped, [], |row| row.get(0).map_err(Box::new))
        .map_err(|err: Box<Error>| err)?;
    Ok(from_str(&json)?)
}

pub(super) const fn two() -> [Seed<'static>; 2] {
    [
        (KEY, "u1", "c1", "server down"),
        (SECOND, "u1", "c1", "still down"),
    ]
}

pub(super) const fn two_chats() -> [Seed<'static>; 2] {
    [
        (KEY, "u1", "c1", "server down"),
        (SECOND, "u1", "c2", "still down"),
    ]
}
