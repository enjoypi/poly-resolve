use std::path::Path;

use rusqlite::{Connection, Error, params};

const INSERT: &str = "INSERT OR IGNORE INTO messages (key, source, account, conversation_id, sender_id, sender_name, text, occurred_at_ms, received_at_ms, raw) SELECT json_extract(value, '$.key'), json_extract(value, '$.source'), json_extract(value, '$.account'), json_extract(value, '$.conversation_id'), json_extract(value, '$.sender_id'), json_extract(value, '$.sender_name'), json_extract(value, '$.text'), json_extract(value, '$.occurred_at_ms'), json_extract(value, '$.received_at_ms'), json_extract(value, '$.raw') FROM json_each(?1, ?2)";
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

pub type Fallible<T> = Result<T, Box<Error>>;
