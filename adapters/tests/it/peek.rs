use std::path::Path;

use rusqlite::Connection;
use serde_json::from_str;

use crate::support::Fallible;

pub(crate) fn health(database: &Path) -> Fallible<Vec<String>> {
    let connection = Connection::open(database)?;
    let json: String = connection.query_row_and_then(
        "SELECT json_group_array(source || '|' || account || '|' || state || '|' || streak) FROM source_health ORDER BY source, account",
        [],
        |row| row.get(0).map_err(Box::new),
    )?;
    Ok(from_str(&json)?)
}

pub(crate) fn keys(database: &Path) -> Fallible<Vec<String>> {
    let connection = Connection::open(database)?;
    let mut statement = connection.prepare("SELECT key FROM messages ORDER BY key")?;
    let mut rows = statement.query([])?;
    let mut keys = Vec::new();
    while let Some(row) = rows.next()? {
        keys.push(row.get(0)?);
    }
    Ok(keys)
}

pub(crate) fn texts(database: &Path) -> Fallible<Vec<String>> {
    let connection = Connection::open(database)?;
    let mut statement = connection.prepare("SELECT text FROM messages ORDER BY key")?;
    let mut rows = statement.query([])?;
    let mut texts = Vec::new();
    while let Some(row) = rows.next()? {
        texts.push(row.get(0)?);
    }
    Ok(texts)
}
