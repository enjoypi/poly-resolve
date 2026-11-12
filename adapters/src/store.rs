use core::error::Error as Cause;
use core::time::Duration;
use std::path::Path;

use domain::AppError;
use domain::batches::{Item as Batch, Roster};
use domain::inbox::{Mark, Query, Reader, Row};
use domain::outbound::{Item, Listing, Outbox};
use rusqlite::{Connection, Error, OpenFlags, params};
use serde_json::{Value, from_str};

use crate::ingest::now_ms;

const ATTEMPTS: Column = Column {
    alter: "ALTER TABLE lines ADD COLUMN attempts INTEGER NOT NULL DEFAULT 0",
    name: "attempts",
    table: "lines",
};
const BACKFILL: &str = "INSERT OR IGNORE INTO backfills (source, account, version, done_at_ms) VALUES (?1, ?2, ?3, ?4)";
const BACKFILLED: &str =
    "SELECT COUNT(*) FROM backfills WHERE source = ?1 AND account = ?2 AND version = ?3";
const BATCHES: &str = "SELECT json_group_array(json_object('attempts', attempts, 'batch_id', id, 'lines', (SELECT COUNT(*) FROM lines l WHERE l.batch_id = b.id AND l.state = 'approved'), 'reason', reason, 'state', state, 'updated_at_ms', updated_at_ms)) FROM (SELECT * FROM batches WHERE state = ?1 ORDER BY id LIMIT ?2) b";
const BATCH_STATE: &str = "SELECT json_group_array(state) FROM batches WHERE id = ?1";
const CHANGES: Column = Column {
    alter: "ALTER TABLE alerts ADD COLUMN changes TEXT NOT NULL DEFAULT ''",
    name: "changes",
    table: "alerts",
};
static COLUMNS: [Column; 7] = [ATTEMPTS, CHANGES, CURSOR, REVIVALS, ROLE, SCORE, STATE];
const CURSOR: Column = Column {
    alter: "ALTER TABLE topics ADD COLUMN until_key TEXT NOT NULL DEFAULT ''",
    name: "until_key",
    table: "topics",
};
const INSERT: &str = "INSERT OR IGNORE INTO messages (key, source, account, conversation_id, sender_id, sender_name, text, occurred_at_ms, received_at_ms, raw) SELECT json_extract(value, '$.key'), json_extract(value, '$.source'), json_extract(value, '$.account'), json_extract(value, '$.conversation_id'), json_extract(value, '$.sender_id'), json_extract(value, '$.sender_name'), json_extract(value, '$.text'), json_extract(value, '$.occurred_at_ms'), json_extract(value, '$.received_at_ms'), json_extract(value, '$.raw') FROM json_each(?1, ?2)";
const LIST: &str = "SELECT json_group_array(json_object('account', account, 'analysis', analysis, 'conversation_id', conversation_id, 'key', key, 'occurred_at_ms', occurred_at_ms, 'priority', priority, 'reason', reason, 'received_at_ms', received_at_ms, 'sender_id', sender_id, 'sender_name', sender_name, 'source', source, 'state', state, 'text', text, 'topic', topic) ORDER BY occurred_at_ms DESC, key DESC) FROM (SELECT m.*, COALESCE(t.score, p.probability) AS priority, l.reason AS reason, l.state AS state, p.topic_id AS topic, (SELECT a.analysis FROM alerts a WHERE a.topic_id = p.topic_id AND a.analysis IS NOT NULL ORDER BY a.created_at_ms DESC LIMIT 1) AS analysis FROM messages m LEFT JOIN members p ON p.key = m.key AND p.topic_id = (SELECT MIN(o.topic_id) FROM members o WHERE o.key = m.key) LEFT JOIN topics t ON t.id = p.topic_id LEFT JOIN lines l ON l.key = m.key WHERE (?1 IS NULL OR (m.occurred_at_ms, m.key) < (?1, ?2)) AND (?4 IS NULL OR m.occurred_at_ms >= ?4) AND (?5 IS NULL OR m.occurred_at_ms < ?5) ORDER BY m.occurred_at_ms DESC, m.key DESC LIMIT ?3)";
const OUTBOUND: &str = "SELECT json_group_array(json_object('account', account, 'batch_id', batch_id, 'conversation_id', conversation_id, 'flag', flag, 'key', key, 'occurred_at_ms', occurred_at_ms, 'reason', reason, 'role', role, 'sender', sender, 'source', source, 'state', state, 'text', text)) FROM (SELECT l.*, COALESCE(m.source, '') AS source, COALESCE(m.account, '') AS account FROM lines l LEFT JOIN messages m ON m.key = l.key WHERE l.state = ?1 ORDER BY l.occurred_at_ms DESC, l.key DESC LIMIT ?2)";
const PRESENT: &str = "SELECT COUNT(*) FROM pragma_table_info(?1) WHERE name = ?2";
const REPLACE: &str = "UPDATE messages SET text = json_extract(?2, '$.text') WHERE key = json_extract(?2, '$.key') AND text = json_extract(?1, '$.text')";
const REVIVALS: Column = Column {
    alter: "ALTER TABLE batches ADD COLUMN revivals INTEGER NOT NULL DEFAULT 0; UPDATE batches SET revivals = -1 WHERE state = 'failed'",
    name: "revivals",
    table: "batches",
};
const REVIVE: &str = "UPDATE batches SET state = 'ready', revivals = MAX(revivals, 0) + 1, reason = '', updated_at_ms = 0 WHERE id = ?1 AND state = 'failed' AND NOT EXISTS (SELECT 1 FROM batches WHERE state = 'ready')";
const ROLE: Column = Column {
    alter: "ALTER TABLE lines ADD COLUMN role TEXT NOT NULL DEFAULT ''",
    name: "role",
    table: "lines",
};
pub(crate) const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS messages (key TEXT PRIMARY KEY NOT NULL, source TEXT NOT NULL, account TEXT NOT NULL, conversation_id TEXT NOT NULL, sender_id TEXT NOT NULL, sender_name TEXT NOT NULL, text TEXT NOT NULL, occurred_at_ms INTEGER NOT NULL, received_at_ms INTEGER NOT NULL, raw TEXT NOT NULL) WITHOUT ROWID; CREATE INDEX IF NOT EXISTS messages_by_time ON messages (source, account, occurred_at_ms); CREATE INDEX IF NOT EXISTS messages_by_when ON messages (occurred_at_ms, key); CREATE TABLE IF NOT EXISTS lines (key TEXT PRIMARY KEY NOT NULL, conversation_id TEXT NOT NULL, occurred_at_ms INTEGER NOT NULL, sender TEXT NOT NULL, text TEXT NOT NULL, state TEXT NOT NULL, reason TEXT NOT NULL, reviewer TEXT NOT NULL, flag TEXT NOT NULL, hints TEXT NOT NULL, rules_rev INTEGER NOT NULL, batch_id INTEGER NOT NULL, created_at_ms INTEGER NOT NULL, updated_at_ms INTEGER NOT NULL, attempts INTEGER NOT NULL DEFAULT 0, role TEXT NOT NULL DEFAULT '') WITHOUT ROWID; CREATE TABLE IF NOT EXISTS misses (id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, kind TEXT NOT NULL, key TEXT NOT NULL, text TEXT NOT NULL, spans TEXT NOT NULL, created_at_ms INTEGER NOT NULL); CREATE INDEX IF NOT EXISTS lines_by_batch ON lines (batch_id, state); CREATE INDEX IF NOT EXISTS lines_by_state ON lines (state, occurred_at_ms); CREATE INDEX IF NOT EXISTS lines_by_conversation ON lines (conversation_id, occurred_at_ms); CREATE TABLE IF NOT EXISTS answers (batch_id INTEGER PRIMARY KEY NOT NULL, body TEXT NOT NULL); CREATE TABLE IF NOT EXISTS batches (id INTEGER PRIMARY KEY NOT NULL, state TEXT NOT NULL, payload TEXT NOT NULL, layout TEXT NOT NULL, attempts INTEGER NOT NULL, reason TEXT NOT NULL, created_at_ms INTEGER NOT NULL, updated_at_ms INTEGER NOT NULL, revivals INTEGER NOT NULL DEFAULT 0); CREATE TABLE IF NOT EXISTS topics (id TEXT PRIMARY KEY NOT NULL, label TEXT NOT NULL, probability REAL, thread_id TEXT NOT NULL, until_ms INTEGER NOT NULL, until_key TEXT NOT NULL, thread_at_ms INTEGER NOT NULL, created_at_ms INTEGER NOT NULL, last_at_ms INTEGER NOT NULL) WITHOUT ROWID; CREATE TABLE IF NOT EXISTS placements (key TEXT PRIMARY KEY NOT NULL, topic_id TEXT NOT NULL, probability REAL, batch_id INTEGER NOT NULL) WITHOUT ROWID; CREATE INDEX IF NOT EXISTS placements_by_topic ON placements (topic_id); CREATE TABLE IF NOT EXISTS alerts (key TEXT PRIMARY KEY NOT NULL, topic_id TEXT NOT NULL, state TEXT NOT NULL, attempts INTEGER NOT NULL, analysis TEXT, reason TEXT NOT NULL, created_at_ms INTEGER NOT NULL, updated_at_ms INTEGER NOT NULL, score REAL) WITHOUT ROWID; CREATE INDEX IF NOT EXISTS alerts_by_topic ON alerts (topic_id, created_at_ms); CREATE TABLE IF NOT EXISTS learned_rules (id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, pattern TEXT NOT NULL, placeholder TEXT NOT NULL, source_key TEXT NOT NULL, reviewer TEXT NOT NULL, created_at_ms INTEGER NOT NULL); CREATE UNIQUE INDEX IF NOT EXISTS learned_rules_unique ON learned_rules (pattern, placeholder); CREATE TABLE IF NOT EXISTS topic_aliases (old TEXT PRIMARY KEY NOT NULL, new TEXT NOT NULL) WITHOUT ROWID; CREATE TABLE IF NOT EXISTS triage_state (id INTEGER PRIMARY KEY NOT NULL, enabled INTEGER NOT NULL, since_ms INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS calls (kind TEXT NOT NULL, subject TEXT NOT NULL, topic_id TEXT NOT NULL, attempt INTEGER NOT NULL, at_ms INTEGER NOT NULL, PRIMARY KEY (kind, subject, attempt)) WITHOUT ROWID; CREATE INDEX IF NOT EXISTS calls_by_time ON calls (kind, at_ms); CREATE INDEX IF NOT EXISTS calls_by_topic ON calls (topic_id, at_ms); CREATE TABLE IF NOT EXISTS backfills (source TEXT NOT NULL, account TEXT NOT NULL, version INTEGER NOT NULL, done_at_ms INTEGER NOT NULL, PRIMARY KEY (source, account, version)) WITHOUT ROWID; CREATE TABLE IF NOT EXISTS members (key TEXT NOT NULL, topic_id TEXT NOT NULL, probability REAL, batch_id INTEGER NOT NULL, via TEXT NOT NULL, created_at_ms INTEGER NOT NULL, PRIMARY KEY (key, topic_id)) WITHOUT ROWID; CREATE INDEX IF NOT EXISTS members_by_topic ON members (topic_id); CREATE INDEX IF NOT EXISTS members_by_time ON members (created_at_ms); CREATE TABLE IF NOT EXISTS clues (kind TEXT NOT NULL, value TEXT NOT NULL, key TEXT NOT NULL, occurred_at_ms INTEGER NOT NULL, PRIMARY KEY (kind, value, key)) WITHOUT ROWID; CREATE INDEX IF NOT EXISTS clues_by_key ON clues (key); CREATE TABLE IF NOT EXISTS relations (a TEXT NOT NULL, b TEXT NOT NULL, kind TEXT NOT NULL, count INTEGER NOT NULL, PRIMARY KEY (a, b, kind)) WITHOUT ROWID; DROP TRIGGER IF EXISTS messages_unsealed; DROP TRIGGER IF EXISTS messages_revealed; CREATE TRIGGER IF NOT EXISTS messages_decrypted AFTER UPDATE OF text ON messages BEGIN DELETE FROM placements WHERE key = NEW.key AND EXISTS (SELECT 1 FROM lines WHERE key = NEW.key AND state = 'withheld' AND reason = 'sealed'); DELETE FROM members WHERE key = NEW.key AND EXISTS (SELECT 1 FROM lines WHERE key = NEW.key AND state = 'withheld' AND reason = 'sealed'); DELETE FROM clues WHERE key = NEW.key AND EXISTS (SELECT 1 FROM lines WHERE key = NEW.key AND state = 'withheld' AND reason = 'sealed'); DELETE FROM lines WHERE key = NEW.key AND state = 'withheld' AND reason = 'sealed'; END;";
const SCORE: Column = Column {
    alter: "ALTER TABLE alerts ADD COLUMN score REAL",
    name: "score",
    table: "alerts",
};
const SEALED: &str = "SELECT json_group_array(json_object('account', account, 'conversation_id', conversation_id, 'key', key, 'occurred_at_ms', occurred_at_ms, 'raw', raw, 'received_at_ms', received_at_ms, 'sender_id', sender_id, 'sender_name', sender_name, 'source', source, 'text', text)) FROM (SELECT * FROM messages WHERE source = ?1 AND account = ?2 AND text GLOB '*||[0-9]*||[0-9]*||[0-9]*' ORDER BY occurred_at_ms DESC, key DESC LIMIT ?3)";
const STATE: Column = Column {
    alter: "ALTER TABLE topics ADD COLUMN state TEXT NOT NULL DEFAULT 'open'; ALTER TABLE topics ADD COLUMN urgency REAL; ALTER TABLE topics ADD COLUMN reach TEXT NOT NULL DEFAULT '{}'; ALTER TABLE topics ADD COLUMN score REAL; ALTER TABLE topics ADD COLUMN verdict TEXT NOT NULL DEFAULT ''; ALTER TABLE topics ADD COLUMN closed_at_ms INTEGER NOT NULL DEFAULT 0",
    name: "state",
    table: "topics",
};
const TUNE: &str = "PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;";
const WATERMARK: &str =
    "SELECT MAX(occurred_at_ms) FROM messages WHERE source = ?1 AND account = ?2";

#[derive(Debug)]
pub struct Archive {
    connection: Connection,
}

impl Archive {
    pub fn backfilled(&self, scope: Scope<'_>, version: u32) -> Fallible<bool> {
        let done: i64 = self.connection.query_row_and_then(
            BACKFILLED,
            params![scope.source, scope.account, i64::from(version)],
            |row| row.get(0).map_err(Box::new),
        )?;
        Ok(done > 0)
    }

    pub fn mark_backfilled(&self, scope: Scope<'_>, version: u32) -> Fallible<usize> {
        self.connection
            .execute(
                BACKFILL,
                params![
                    scope.source,
                    scope.account,
                    i64::from(version),
                    signed(now_ms())
                ],
            )
            .map_err(Box::new)
    }

    pub fn open(path: &Path, busy_ms: u64) -> Fallible<Self> {
        Ok(Self {
            connection: connect(path, busy_ms)?,
        })
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

#[derive(Clone, Copy)]
struct Column {
    alter: &'static str,
    name: &'static str,
    table: &'static str,
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
    busy_ms: u64,
    path: &'shelf Path,
}

impl<'shelf> Shelf<'shelf> {
    fn batches(&self, listing: &Listing<'_>) -> Result<Vec<Batch>, Failure> {
        let connection = self.open(OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let json: String = connection.query_row_and_then(
            BATCHES,
            params![listing.state(), i64::from(listing.limit())],
            |row| row.get(0).map_err(Box::new),
        )?;
        Ok(from_str(&json)?)
    }

    fn fetch(&self, query: &Query) -> Result<Vec<Row>, Failure> {
        let connection = self.open(OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let mark = query.cursor();
        let window = query.window();
        let json: String = connection.query_row_and_then(
            LIST,
            params![
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
    pub const fn new(path: &'shelf Path, busy_ms: u64) -> Self {
        Self { busy_ms, path }
    }

    fn open(&self, flags: OpenFlags) -> Fallible<Connection> {
        let connection = Connection::open_with_flags(self.path, flags).map_err(Box::new)?;
        connection
            .busy_timeout(Duration::from_millis(self.busy_ms))
            .unwrap_or_default();
        Ok(connection)
    }

    fn outbound(&self, listing: &Listing<'_>) -> Result<Vec<Item>, Failure> {
        let connection = self.open(OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let json: String = connection.query_row_and_then(
            OUTBOUND,
            params![listing.state(), i64::from(listing.limit())],
            |row| row.get(0).map_err(Box::new),
        )?;
        Ok(from_str(&json)?)
    }

    fn revived(&self, batch_id: u64) -> Result<Option<String>, Failure> {
        let connection = self.open(OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        let batch = signed(batch_id);
        let json: String = connection
            .execute(REVIVE, params![batch])
            .map_err(Box::new)
            .and_then(|_changed| {
                connection.query_row_and_then(BATCH_STATE, params![batch], |row| {
                    row.get(0).map_err(Box::new)
                })
            })?;
        let states: Vec<String> = from_str(&json).unwrap_or_default();
        Ok(states.into_iter().next())
    }
}

impl Outbox for Shelf<'_> {
    fn list(&self, listing: &Listing<'_>) -> Result<Vec<Item>, AppError> {
        self.outbound(listing).map_err(|err| {
            tracing::warn!(error = %err, "outbound read failed");
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

impl Roster for Shelf<'_> {
    fn list(&self, listing: &Listing<'_>) -> Result<Vec<Batch>, AppError> {
        self.batches(listing).map_err(|err| {
            tracing::warn!(error = %err, "batches read failed");
            AppError::Storage
        })
    }

    fn revive(&self, batch_id: u64) -> Result<Option<String>, AppError> {
        self.revived(batch_id).map_err(|err| {
            tracing::warn!(error = %err, "batch revive failed");
            AppError::Storage
        })
    }
}

pub(crate) fn connect(path: &Path, busy_ms: u64) -> Fallible<Connection> {
    let connection = Connection::open(path).map_err(Box::new)?;
    connection
        .execute_batch(&format!("PRAGMA busy_timeout = {busy_ms}; {TUNE} {SCHEMA}"))
        .map_err(Box::new)?;
    COLUMNS
        .iter()
        .try_for_each(|column| ensure(&connection, *column))?;
    Ok(connection)
}

fn ensure(connection: &Connection, column: Column) -> Fallible<()> {
    let present: i64 =
        connection.query_row_and_then(PRESENT, params![column.table, column.name], |row| {
            row.get(0).map_err(Box::new)
        })?;
    if present == 0 {
        connection.execute_batch(column.alter).map_err(Box::new)?;
    }
    Ok(())
}

fn signed(millis: u64) -> i64 {
    i64::try_from(millis).unwrap_or(i64::MAX)
}
