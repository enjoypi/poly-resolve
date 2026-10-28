use std::path::Path;

use rusqlite::{Connection, Error};
use serde_json::{Value, from_str};

use crate::store::{Fallible, SCHEMA};

const ADVANCE: &str = "UPDATE handoffs SET state = json_extract(?1, '$.state'), analysis = COALESCE(json_extract(?1, '$.analysis'), analysis), reason = '', attempts = 0, updated_at_ms = 0 WHERE key = json_extract(?1, '$.key')";
const CONVERSATION: &str = "SELECT json_group_array(json_object('occurred_at_ms', occurred_at_ms, 'sender_id', sender_id, 'sender_name', sender_name, 'text', text)) FROM (SELECT * FROM messages WHERE source = json_extract(?1, '$.source') AND account = json_extract(?1, '$.account') AND conversation_id = json_extract(?1, '$.conversation_id') AND occurred_at_ms <= json_extract(?1, '$.until_ms') ORDER BY occurred_at_ms DESC, key DESC LIMIT json_extract(?1, '$.limit'))";
const DECISIONS_SINCE: &str = "SELECT json_group_array(json_object('count', total)) FROM (SELECT COUNT(*) AS total FROM decisions WHERE state != 'skipped' AND decided_at_ms >= json_extract(?1, '$.since_ms'))";
const FAIL: &str = "INSERT INTO decisions (key, state, probability, attempts, reason, decided_at_ms) VALUES (json_extract(?1, '$.key'), 'failed', NULL, 1, json_extract(?1, '$.reason'), json_extract(?1, '$.at_ms')) ON CONFLICT(key) DO UPDATE SET state = 'failed', attempts = attempts + 1, reason = excluded.reason, decided_at_ms = excluded.decided_at_ms";
const FAIL_HANDOFF: &str = "UPDATE handoffs SET attempts = attempts + 1, reason = json_extract(?1, '$.reason'), updated_at_ms = json_extract(?1, '$.at_ms'), state = CASE WHEN attempts + 1 >= json_extract(?1, '$.max_attempts') THEN 'failed' ELSE state END WHERE key = json_extract(?1, '$.key')";
const NEXT_HANDOFF: &str = "SELECT json_group_array(json_object('analysis', analysis, 'attempts', attempts, 'conversation_id', conversation_id, 'key', key, 'probability', probability, 'trigger_ms', trigger_ms)) FROM (SELECT h.analysis AS analysis, h.attempts AS attempts, h.conversation_id AS conversation_id, h.key AS key, d.probability AS probability, m.occurred_at_ms AS trigger_ms FROM handoffs h JOIN messages m ON m.key = h.key LEFT JOIN decisions d ON d.key = h.key WHERE h.state = json_extract(?1, '$.state') AND h.updated_at_ms <= json_extract(?1, '$.retry_before_ms') ORDER BY h.created_at_ms, h.key LIMIT 1)";
const OPEN_HANDOFF: &str = "INSERT OR IGNORE INTO handoffs (key, conversation_id, state, attempts, analysis, reason, created_at_ms, updated_at_ms) SELECT json_extract(?1, '$.key'), json_extract(?1, '$.conversation_id'), 'pending', 0, NULL, '', json_extract(?1, '$.at_ms'), 0 WHERE NOT EXISTS (SELECT 1 FROM handoffs WHERE conversation_id = json_extract(?1, '$.conversation_id') AND created_at_ms > json_extract(?1, '$.cooldown_since_ms')) AND (SELECT COUNT(*) FROM handoffs WHERE created_at_ms >= json_extract(?1, '$.hour_since_ms')) < json_extract(?1, '$.max_per_hour')";
const PENDING: &str = "SELECT json_group_array(json_object('attempts', attempts, 'conversation_id', conversation_id, 'key', key, 'sender_id', sender_id, 'text', text)) FROM (SELECT COALESCE(d.attempts, 0) AS attempts, m.conversation_id AS conversation_id, m.key AS key, m.sender_id AS sender_id, m.text AS text FROM messages m LEFT JOIN decisions d ON d.key = m.key WHERE m.source = json_extract(?1, '$.source') AND m.account = json_extract(?1, '$.account') AND m.occurred_at_ms >= json_extract(?1, '$.since_ms') AND (d.key IS NULL OR (d.state = 'failed' AND d.attempts < json_extract(?1, '$.max_attempts') AND d.decided_at_ms <= json_extract(?1, '$.retry_before_ms'))) ORDER BY m.occurred_at_ms, m.key LIMIT json_extract(?1, '$.limit'))";
const SETTLE: &str = "INSERT INTO decisions (key, state, probability, attempts, reason, decided_at_ms) VALUES (json_extract(?1, '$.key'), json_extract(?1, '$.state'), json_extract(?1, '$.probability'), 0, json_extract(?1, '$.reason'), json_extract(?1, '$.at_ms')) ON CONFLICT(key) DO UPDATE SET state = excluded.state, probability = excluded.probability, reason = excluded.reason, decided_at_ms = excluded.decided_at_ms";

pub(crate) struct Ledger {
    connection: Connection,
}

impl Ledger {
    pub(crate) fn advance(&self, args: &Value) -> usize {
        self.exec(ADVANCE, args)
    }

    pub(crate) fn conversation(&self, args: &Value) -> Vec<Value> {
        self.rows(CONVERSATION, args)
    }

    pub(crate) fn decisions_since(&self, args: &Value) -> u64 {
        self.rows(DECISIONS_SINCE, args)
            .first()
            .and_then(|row| row.get("count"))
            .and_then(Value::as_u64)
            .unwrap_or_default()
    }

    fn exec(&self, sql: &str, args: &Value) -> usize {
        self.connection
            .execute(sql, [args.to_string()])
            .unwrap_or_else(|err| {
                tracing::warn!(error = %err, "ledger write failed");
                0
            })
    }

    pub(crate) fn fail(&self, args: &Value) -> usize {
        self.exec(FAIL, args)
    }

    pub(crate) fn fail_handoff(&self, args: &Value) -> usize {
        self.exec(FAIL_HANDOFF, args)
    }

    pub(crate) fn next_handoff(&self, args: &Value) -> Vec<Value> {
        self.rows(NEXT_HANDOFF, args)
    }

    pub(crate) fn open(path: &Path) -> Fallible<Self> {
        let connection = Connection::open(path).map_err(Box::new)?;
        connection.execute_batch(SCHEMA).map_err(Box::new)?;
        Ok(Self { connection })
    }

    pub(crate) fn open_handoff(&self, args: &Value) -> usize {
        self.exec(OPEN_HANDOFF, args)
    }

    pub(crate) fn pending(&self, args: &Value) -> Vec<Value> {
        self.rows(PENDING, args)
    }

    fn rows(&self, sql: &str, args: &Value) -> Vec<Value> {
        let found: Result<String, Box<Error>> =
            self.connection
                .query_row_and_then(sql, [args.to_string()], |row| row.get(0).map_err(Box::new));
        found.map_or_else(
            |err| {
                tracing::warn!(error = %err, "ledger read failed");
                Vec::new()
            },
            |json| from_str(&json).unwrap_or_default(),
        )
    }

    pub(crate) fn settle(&self, args: &Value) -> usize {
        self.exec(SETTLE, args)
    }
}
