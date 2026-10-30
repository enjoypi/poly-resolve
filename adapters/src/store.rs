use core::error::Error as Cause;
use std::path::Path;

use domain::AppError;
use domain::inbox::{Mark, Query, Reader, Row};
use domain::review::{Docket, Item, Listing, Ruling};
use rusqlite::{Connection, Error, OpenFlags, params};
use serde_json::{Value, from_str};

use crate::ingest::now_ms;

const INSERT: &str = "INSERT OR IGNORE INTO messages (key, source, account, conversation_id, sender_id, sender_name, text, occurred_at_ms, received_at_ms, raw) SELECT json_extract(value, '$.key'), json_extract(value, '$.source'), json_extract(value, '$.account'), json_extract(value, '$.conversation_id'), json_extract(value, '$.sender_id'), json_extract(value, '$.sender_name'), json_extract(value, '$.text'), json_extract(value, '$.occurred_at_ms'), json_extract(value, '$.received_at_ms'), json_extract(value, '$.raw') FROM json_each(?1, ?2)";
const LEARN: &str = "INSERT OR IGNORE INTO learned_rules (pattern, placeholder, source_key, reviewer, created_at_ms) SELECT ?1, ?2, ?3, '', ?4 WHERE EXISTS (SELECT 1 FROM lines WHERE key = ?3 AND state = 'pending')";
const LIST: &str = "SELECT json_group_array(json_object('account', account, 'analysis', analysis, 'conversation_id', conversation_id, 'key', key, 'occurred_at_ms', occurred_at_ms, 'priority', priority, 'received_at_ms', received_at_ms, 'sender_id', sender_id, 'sender_name', sender_name, 'source', source, 'text', text, 'topic', topic) ORDER BY occurred_at_ms DESC, key DESC) FROM (SELECT m.*, p.probability AS priority, p.topic_id AS topic, (SELECT a.analysis FROM alerts a WHERE a.topic_id = p.topic_id AND a.analysis IS NOT NULL ORDER BY a.created_at_ms DESC LIMIT 1) AS analysis FROM messages m LEFT JOIN placements p ON p.key = m.key WHERE m.source = ?1 AND m.account = ?2 AND (?3 IS NULL OR (m.occurred_at_ms, m.key) < (?3, ?4)) AND (?6 IS NULL OR m.occurred_at_ms >= ?6) AND (?7 IS NULL OR m.occurred_at_ms < ?7) ORDER BY m.occurred_at_ms DESC, m.key DESC LIMIT ?5)";
const REPLACE: &str = "UPDATE messages SET text = json_extract(?2, '$.text') WHERE key = json_extract(?2, '$.key') AND text = json_extract(?1, '$.text')";
const REVIEWS: &str = "SELECT json_group_array(json_object('batch_id', batch_id, 'conversation_id', conversation_id, 'flag', flag, 'key', key, 'occurred_at_ms', occurred_at_ms, 'reason', reason, 'sender', sender, 'state', state, 'text', text)) FROM (SELECT * FROM lines WHERE state = ?1 AND batch_id > 0 ORDER BY flag = '', batch_id, occurred_at_ms, key LIMIT ?2)";
pub(crate) const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS messages (key TEXT PRIMARY KEY NOT NULL, source TEXT NOT NULL, account TEXT NOT NULL, conversation_id TEXT NOT NULL, sender_id TEXT NOT NULL, sender_name TEXT NOT NULL, text TEXT NOT NULL, occurred_at_ms INTEGER NOT NULL, received_at_ms INTEGER NOT NULL, raw TEXT NOT NULL) WITHOUT ROWID; CREATE INDEX IF NOT EXISTS messages_by_time ON messages (source, account, occurred_at_ms); CREATE TABLE IF NOT EXISTS lines (key TEXT PRIMARY KEY NOT NULL, conversation_id TEXT NOT NULL, occurred_at_ms INTEGER NOT NULL, sender TEXT NOT NULL, text TEXT NOT NULL, state TEXT NOT NULL, reason TEXT NOT NULL, reviewer TEXT NOT NULL, flag TEXT NOT NULL, hints TEXT NOT NULL, rules_rev INTEGER NOT NULL, batch_id INTEGER NOT NULL, created_at_ms INTEGER NOT NULL, updated_at_ms INTEGER NOT NULL) WITHOUT ROWID; CREATE INDEX IF NOT EXISTS lines_by_batch ON lines (batch_id, state); CREATE INDEX IF NOT EXISTS lines_by_state ON lines (state, occurred_at_ms); CREATE TABLE IF NOT EXISTS batches (id INTEGER PRIMARY KEY NOT NULL, state TEXT NOT NULL, payload TEXT NOT NULL, layout TEXT NOT NULL, attempts INTEGER NOT NULL, reason TEXT NOT NULL, created_at_ms INTEGER NOT NULL, updated_at_ms INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS topics (id TEXT PRIMARY KEY NOT NULL, label TEXT NOT NULL, probability REAL, thread_id TEXT NOT NULL, until_ms INTEGER NOT NULL, thread_at_ms INTEGER NOT NULL, created_at_ms INTEGER NOT NULL, last_at_ms INTEGER NOT NULL) WITHOUT ROWID; CREATE TABLE IF NOT EXISTS placements (key TEXT PRIMARY KEY NOT NULL, topic_id TEXT NOT NULL, probability REAL, batch_id INTEGER NOT NULL) WITHOUT ROWID; CREATE INDEX IF NOT EXISTS placements_by_topic ON placements (topic_id); CREATE TABLE IF NOT EXISTS alerts (key TEXT PRIMARY KEY NOT NULL, topic_id TEXT NOT NULL, state TEXT NOT NULL, attempts INTEGER NOT NULL, analysis TEXT, reason TEXT NOT NULL, created_at_ms INTEGER NOT NULL, updated_at_ms INTEGER NOT NULL) WITHOUT ROWID; CREATE INDEX IF NOT EXISTS alerts_by_topic ON alerts (topic_id, created_at_ms); CREATE TABLE IF NOT EXISTS learned_rules (id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, pattern TEXT NOT NULL, placeholder TEXT NOT NULL, source_key TEXT NOT NULL, reviewer TEXT NOT NULL, created_at_ms INTEGER NOT NULL); CREATE UNIQUE INDEX IF NOT EXISTS learned_rules_unique ON learned_rules (pattern, placeholder); CREATE TRIGGER IF NOT EXISTS messages_unsealed AFTER UPDATE OF text ON messages BEGIN DELETE FROM lines WHERE key = NEW.key AND state = 'skipped' AND reason = 'sealed'; END;";
const SEALED: &str = "SELECT json_group_array(json_object('account', account, 'conversation_id', conversation_id, 'key', key, 'occurred_at_ms', occurred_at_ms, 'raw', raw, 'received_at_ms', received_at_ms, 'sender_id', sender_id, 'sender_name', sender_name, 'source', source, 'text', text)) FROM (SELECT * FROM messages WHERE source = ?1 AND account = ?2 AND text GLOB '*||[0-9]*||[0-9]*||[0-9]*' ORDER BY occurred_at_ms DESC, key DESC LIMIT ?3)";
const SETTLE: &str = "UPDATE lines SET state = ?2, reason = ?3, updated_at_ms = ?4 WHERE key = ?1 AND state = 'pending'";
const SOURCE: &str = "SELECT json_group_array(text) FROM lines WHERE key = ?1";
const STATE: &str = "SELECT json_group_array(state) FROM lines WHERE key = ?1";
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

    fn reviews(&self, listing: &Listing<'_>) -> Result<Vec<Item>, Failure> {
        let connection = Connection::open_with_flags(self.path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let json: String = connection.query_row_and_then(
            REVIEWS,
            params![listing.state(), i64::from(listing.limit())],
            |row| row.get(0).map_err(Box::new),
        )?;
        Ok(from_str(&json)?)
    }

    fn rule(&self, ruling: &Ruling<'_>) -> Result<Option<String>, Failure> {
        let connection = Connection::open_with_flags(self.path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        let now = signed(now_ms());
        let json: String = learn(&connection, ruling, now)
            .and_then(|_changed| {
                connection
                    .execute(
                        SETTLE,
                        params![ruling.key(), ruling.state(), ruling.reason(), now],
                    )
                    .map_err(Box::new)
            })
            .and_then(|_changed| {
                connection.query_row_and_then(STATE, params![ruling.key()], |row| {
                    row.get(0).map_err(Box::new)
                })
            })?;
        let states: Vec<String> = from_str(&json)?;
        Ok(states.into_iter().next())
    }

    fn text_of(&self, key: &str) -> Result<Option<String>, Failure> {
        let connection = Connection::open_with_flags(self.path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let json: String = connection
            .query_row_and_then(SOURCE, params![key], |row| row.get(0).map_err(Box::new))?;
        let texts: Vec<String> = from_str(&json)?;
        Ok(texts.into_iter().next())
    }
}

impl Docket for Shelf<'_> {
    fn list(&self, listing: &Listing<'_>) -> Result<Vec<Item>, AppError> {
        self.reviews(listing).map_err(|err| {
            tracing::warn!(error = %err, "reviews read failed");
            AppError::Storage
        })
    }

    fn settle(&self, ruling: &Ruling<'_>) -> Result<Option<String>, AppError> {
        self.rule(ruling).map_err(|err| {
            tracing::warn!(error = %err, "review write failed");
            AppError::Storage
        })
    }

    fn source(&self, key: &str) -> Result<Option<String>, AppError> {
        self.text_of(key).map_err(|err| {
            tracing::warn!(error = %err, "review source read failed");
            AppError::Storage
        })
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

fn learn(connection: &Connection, ruling: &Ruling<'_>, now: i64) -> Fallible<usize> {
    ruling.rule().map_or(Ok(usize::default()), |rule| {
        connection
            .execute(
                LEARN,
                params![rule.pattern(), rule.placeholder(), ruling.key(), now],
            )
            .map_err(Box::new)
    })
}

fn signed(millis: u64) -> i64 {
    i64::try_from(millis).unwrap_or(i64::MAX)
}
