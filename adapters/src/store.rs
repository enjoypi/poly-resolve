use core::error::Error as Cause;
use std::path::Path;

use domain::AppError;
use domain::inbox::{Mark, Query, Reader, Row};
use rusqlite::{Connection, Error, OpenFlags, params};
use serde_json::{Value, from_str};

const INSERT: &str = "INSERT OR IGNORE INTO messages (key, source, account, conversation_id, sender_id, sender_name, text, occurred_at_ms, received_at_ms, raw) SELECT json_extract(value, '$.key'), json_extract(value, '$.source'), json_extract(value, '$.account'), json_extract(value, '$.conversation_id'), json_extract(value, '$.sender_id'), json_extract(value, '$.sender_name'), json_extract(value, '$.text'), json_extract(value, '$.occurred_at_ms'), json_extract(value, '$.received_at_ms'), json_extract(value, '$.raw') FROM json_each(?1, ?2)";
const LIST: &str = "SELECT json_group_array(json_object('account', account, 'analysis', analysis, 'conversation_id', conversation_id, 'key', key, 'occurred_at_ms', occurred_at_ms, 'priority', priority, 'received_at_ms', received_at_ms, 'sender_id', sender_id, 'sender_name', sender_name, 'source', source, 'text', text) ORDER BY occurred_at_ms DESC, key DESC) FROM (SELECT m.*, d.probability AS priority, h.analysis AS analysis FROM messages m LEFT JOIN decisions d ON d.key = m.key LEFT JOIN handoffs h ON h.key = m.key WHERE m.source = ?1 AND m.account = ?2 AND (?3 IS NULL OR (m.occurred_at_ms, m.key) < (?3, ?4)) AND (?6 IS NULL OR m.occurred_at_ms >= ?6) AND (?7 IS NULL OR m.occurred_at_ms < ?7) ORDER BY m.occurred_at_ms DESC, m.key DESC LIMIT ?5)";
const REPLACE: &str = "UPDATE messages SET text = json_extract(?2, '$.text') WHERE key = json_extract(?2, '$.key') AND text = json_extract(?1, '$.text')";
pub(crate) const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS messages (key TEXT PRIMARY KEY NOT NULL, source TEXT NOT NULL, account TEXT NOT NULL, conversation_id TEXT NOT NULL, sender_id TEXT NOT NULL, sender_name TEXT NOT NULL, text TEXT NOT NULL, occurred_at_ms INTEGER NOT NULL, received_at_ms INTEGER NOT NULL, raw TEXT NOT NULL) WITHOUT ROWID; CREATE INDEX IF NOT EXISTS messages_by_time ON messages (source, account, occurred_at_ms); CREATE TABLE IF NOT EXISTS decisions (key TEXT PRIMARY KEY NOT NULL, state TEXT NOT NULL, probability REAL, attempts INTEGER NOT NULL, reason TEXT NOT NULL, decided_at_ms INTEGER NOT NULL) WITHOUT ROWID; CREATE TABLE IF NOT EXISTS handoffs (key TEXT PRIMARY KEY NOT NULL, conversation_id TEXT NOT NULL, state TEXT NOT NULL, attempts INTEGER NOT NULL, analysis TEXT, reason TEXT NOT NULL, created_at_ms INTEGER NOT NULL, updated_at_ms INTEGER NOT NULL) WITHOUT ROWID; CREATE INDEX IF NOT EXISTS handoffs_by_conversation ON handoffs (conversation_id, created_at_ms);";
const SEALED: &str = "SELECT json_group_array(json_object('account', account, 'conversation_id', conversation_id, 'key', key, 'occurred_at_ms', occurred_at_ms, 'raw', raw, 'received_at_ms', received_at_ms, 'sender_id', sender_id, 'sender_name', sender_name, 'source', source, 'text', text)) FROM (SELECT * FROM messages WHERE source = ?1 AND account = ?2 AND text GLOB '*||[0-9]*||[0-9]*||[0-9]*' ORDER BY occurred_at_ms DESC, key DESC LIMIT ?3)";
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

    pub fn replace(&self, stale: &str, fresh: &str) -> Fallible<usize> {
        self.connection
            .execute(REPLACE, params![stale, fresh])
            .map_err(Box::new)
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

    pub fn sealed(&self, scope: Scope<'_>, limit: u32) -> Fallible<Vec<String>> {
        let json: String = self.connection.query_row_and_then(
            SEALED,
            params![scope.source, scope.account, i64::from(limit)],
            |row| row.get(0).map_err(Box::new),
        )?;
        let rows: Vec<Value> = from_str(&json).unwrap_or_default();
        Ok(rows.iter().map(Value::to_string).collect())
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

#[derive(Clone, Copy, Debug)]
pub struct Scope<'scope> {
    account: &'scope str,
    source: &'scope str,
}

impl<'scope> Scope<'scope> {
    #[must_use = "范围需交给查询"]
    pub const fn new(source: &'scope str, account: &'scope str) -> Self {
        Self { account, source }
    }
}

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
        let window = query.window();
        let json: String = connection.query_row_and_then(
            LIST,
            params![
                self.source,
                self.account,
                mark.map(|point| signed(point.occurred_at_ms())),
                mark.map(Mark::key),
                i64::from(query.limit()),
                window.since().map(signed),
                window.until().map(signed)
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

fn signed(millis: u64) -> i64 {
    i64::try_from(millis).unwrap_or(i64::MAX)
}
