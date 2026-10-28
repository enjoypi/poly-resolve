use std::env;
use std::io::ErrorKind;

use adapters::store::Archive;
use rusqlite::{Connection, Error};
use serde_json::{Value, from_str, json};

use crate::support::{Fallible, NOON, Sandbox, triage_logged};

pub(super) const QUICK: u64 = 2_500;
pub(super) const SHORT: u64 = 4_000;

pub(super) type Seed<'seed> = (&'seed str, &'seed str, &'seed str, &'seed str);

pub(super) fn calls(sandbox: &Sandbox, role: &str) -> Fallible<usize> {
    match sandbox.read(&format!("{role}-calls.txt")) {
        Ok(text) => Ok(text.lines().count()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(0),
        Err(err) => Err(err.into()),
    }
}

pub(super) fn home() -> Fallible<String> {
    Ok(env::var("HOME")?)
}

pub(super) async fn logged(sandbox: &Sandbox, patch: &Value, millis: u64) -> Fallible<String> {
    let ingest = sandbox.config(&json!({}))?;
    let triage = sandbox.triage(patch)?;
    triage_logged(&ingest, &triage, millis).await
}

pub(super) fn path() -> Fallible<String> {
    Ok(env::var("PATH")?)
}

pub(super) fn seed(sandbox: &Sandbox, rows: &[Seed<'_>]) -> Fallible<()> {
    let archive = Archive::open(&sandbox.database())?;
    for (index, row) in rows.iter().enumerate() {
        let &(key, sender, conversation, text) = row;
        let stamp = NOON.saturating_add(u64::try_from(index)?);
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
