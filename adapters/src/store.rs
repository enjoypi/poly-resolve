use core::error::Error as Cause;
use std::path::Path;

use domain::AppError;
use domain::inbox::{Mark, Query, Reader, Row};
use rusqlite::{Connection, Error, OpenFlags, params};
use serde_json::from_str;

const INSERT: &str = "INSERT OR IGNORE INTO messages (key, source, account, conversation_id, sender_id, sender_name, text, occurred_at_ms, received_at_ms, raw) SELECT json_extract(value, '$.key'), json_extract(value, '$.source'), json_extract(value, '$.account'), json_extract(value, '$.conversation_id'), json_extract(value, '$.sender_id'), json_extract(value, '$.sender_name'), json_extract(value, '$.text'), json_extract(value, '$.occurred_at_ms'), json_extract(value, '$.received_at_ms'), json_extract(value, '$.raw') FROM json_each(?1, ?2)";
const LIST: &str = "SELECT json_group_array(json_object('account', account, 'conversation_id', conversation_id, 'key', key, 'occurred_at_ms', occurred_at_ms, 'received_at_ms', received_at_ms, 'sender_id', sender_id, 'sender_name', sender_name, 'source', source, 'text', text) ORDER BY occurred_at_ms DESC, key DESC) FROM (SELECT * FROM messages WHERE source = ?1 AND account = ?2 AND (?3 IS NULL OR (occurred_at_ms, key) < (?3, ?4)) ORDER BY occurred_at_ms DESC, key DESC LIMIT ?5)";
const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS messages (key TEXT PRIMARY KEY NOT NULL, source TEXT NOT NULL, account TEXT NOT NULL, conversation_id TEXT NOT NULL, sender_id TEXT NOT NULL, sender_name TEXT NOT NULL, text TEXT NOT NULL, occurred_at_ms INTEGER NOT NULL, received_at_ms INTEGER NOT NULL, raw TEXT NOT NULL) WITHOUT ROWID; CREATE INDEX IF NOT EXISTS messages_by_time ON messages (source, account, occurred_at_ms);";
const WATERMARK: &str =
    "SELECT MAX(occurred_at_ms) FROM messages WHERE source = ?1 AND account = ?2";

#[derive(Debug)]
pub struct Archive {
    connection: Connection,
}

impl Archive {
    pub fn open(path: &Path) -> Fallible<Self> {
        let connection = Connection::open(path).map_err(Box::new)?;
        connection.execute_batch(SCHEMA).map_err(Box::new)?;
        Ok(Self { connection })
    }

    fn save(&self, json: &str, path: &str) -> Fallible<usize> {
        self.connection
            .execute(INSERT, params![json, path])
            .map_err(Box::new)
    }

    pub fn save_event(&self, envelope: &str) -> Fallible<usize> {
        self.save(&format!("[{envelope}]"), "$")
    }

    pub fn save_page(&self, page: &str) -> Fallible<usize> {
        self.save(page, "$.envelopes")
    }

    pub fn watermark(&self, source: &str, account: &str) -> Fallible<Option<u64>> {
        let mark: Option<i64> =
            self.connection
                .query_row_and_then(WATERMARK, params![source, account], |row| {
                    row.get(0).map_err(Box::new)
                })?;
        Ok(mark.map(|millis| u64::try_from(millis).unwrap_or_default()))
    }
}

type Failure = Box<dyn Cause>;

pub type Fallible<T> = Result<T, Box<Error>>;

#[derive(Debug)]
pub struct Shelf<'shelf> {
    account: &'shelf str,
    path: &'shelf Path,
    source: &'shelf str,
}

impl<'shelf> Shelf<'shelf> {
    fn fetch(&self, query: &Query) -> Result<Vec<Row>, Failure> {
        let connection = Connection::open_with_flags(self.path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let mark = query.cursor();
        let json: String = connection.query_row_and_then(
            LIST,
            params![
                self.source,
                self.account,
                mark.map(|point| i64::try_from(point.occurred_at_ms()).unwrap_or(i64::MAX)),
                mark.map(Mark::key),
                i64::from(query.limit())
            ],
            |row| row.get(0).map_err(Box::new),
        )?;
        Ok(from_str(&json)?)
    }

    #[must_use = "货架需交给 Context"]
    pub const fn new(path: &'shelf Path, source: &'shelf str, account: &'shelf str) -> Self {
        Self {
            account,
            path,
            source,
        }
    }
}

impl Reader for Shelf<'_> {
    fn page(&self, query: &Query) -> Result<Vec<Row>, AppError> {
        self.fetch(query).map_err(|err| {
            tracing::warn!(error = %err, "messages read failed");
            AppError::Storage
        })
    }
}
