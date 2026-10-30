use std::path::Path;

use rusqlite::{Connection, Error, params_from_iter};
use serde_json::{Value, from_str};

use crate::store::{Fallible, connect};

const ADVANCE_ALERT: &str = "UPDATE alerts SET state = json_extract(?1, '$.state'), analysis = COALESCE(json_extract(?1, '$.analysis'), analysis), reason = '', attempts = 0, updated_at_ms = 0 WHERE key = json_extract(?1, '$.key') AND state = json_extract(?1, '$.from')";
const ANSWER: &str = "INSERT OR REPLACE INTO answers (batch_id, body) VALUES (json_extract(?1, '$.batch_id'), json_extract(?1, '$.body'))";
const ATTACH: &str = "SELECT json_group_array(json_object('conversation_id', conversation_id, 'last_ms', last_ms, 'topic_id', topic_id)) FROM (SELECT l.conversation_id AS conversation_id, MAX(l.occurred_at_ms) AS last_ms, p.topic_id AS topic_id FROM placements p JOIN lines l ON l.key = p.key JOIN topics t ON t.id = p.topic_id WHERE t.last_at_ms >= json_extract(?1, '$.since_ms') AND l.conversation_id IN (SELECT conversation_id FROM lines WHERE batch_id = json_extract(?1, '$.batch_id')) GROUP BY l.conversation_id)";
const BATCH_CLAIM: &str = "UPDATE batches SET attempts = attempts + 1, updated_at_ms = json_extract(?1, '$.at_ms') WHERE id = json_extract(?1, '$.id') AND state = 'ready' AND attempts < json_extract(?1, '$.max_attempts')";
const BATCH_FAIL: &str = "UPDATE batches SET reason = json_extract(?1, '$.reason'), updated_at_ms = json_extract(?1, '$.at_ms'), state = CASE WHEN attempts >= json_extract(?1, '$.max_attempts') THEN 'failed' ELSE state END WHERE id = json_extract(?1, '$.id') AND state = 'ready'";
const BATCH_LINES: &str = "SELECT json_group_array(json_object('conversation_id', conversation_id, 'hints', json(hints), 'key', key, 'occurred_at_ms', occurred_at_ms, 'sender', sender, 'text', text)) FROM (SELECT * FROM lines WHERE batch_id = json_extract(?1, '$.batch_id') AND state = 'approved' ORDER BY occurred_at_ms, key)";
const BATCH_NEXT: &str = "SELECT json_group_array(json_object('answer', answer, 'id', id, 'layout', layout, 'payload', payload)) FROM (SELECT a.body AS answer, b.id AS id, b.layout AS layout, b.payload AS payload FROM batches b LEFT JOIN answers a ON a.batch_id = b.id WHERE b.state = 'ready' AND b.updated_at_ms <= json_extract(?1, '$.retry_before_ms') AND (a.body IS NOT NULL OR b.attempts < json_extract(?1, '$.max_attempts')) ORDER BY b.id LIMIT 1)";
const BATCH_RELEASE: &str = "UPDATE batches SET attempts = attempts - 1, updated_at_ms = 0 WHERE id = json_extract(?1, '$.id') AND state = 'ready' AND attempts > 0";
const BATCH_SEAL: &str = "INSERT OR IGNORE INTO batches (id, state, payload, layout, attempts, reason, created_at_ms, updated_at_ms) VALUES (json_extract(?1, '$.id'), json_extract(?1, '$.state'), json_extract(?1, '$.payload'), json_extract(?1, '$.layout'), 0, '', json_extract(?1, '$.at_ms'), json_extract(?1, '$.updated_ms'))";
const BATCH_SENT: &str = "UPDATE batches SET state = 'sent', attempts = 0, reason = '', updated_at_ms = json_extract(?1, '$.at_ms') WHERE id = json_extract(?1, '$.batch_id') AND state = 'ready'";
const BEGIN: &str = "BEGIN IMMEDIATE";
const BIND_THREAD: &str = "UPDATE topics SET thread_id = json_extract(?1, '$.thread_id'), until_ms = json_extract(?1, '$.until_ms'), thread_at_ms = CASE WHEN thread_id = json_extract(?1, '$.thread_id') THEN thread_at_ms ELSE json_extract(?1, '$.at_ms') END WHERE id = json_extract(?1, '$.topic_id')";
const CLAIM_ALERT: &str = "UPDATE alerts SET attempts = attempts + 1, updated_at_ms = json_extract(?1, '$.at_ms') WHERE key = json_extract(?1, '$.key') AND state = json_extract(?1, '$.state') AND attempts < json_extract(?1, '$.max_attempts')";
const COMMIT: &str = "COMMIT";
const DEFER: &str = "UPDATE lines SET reason = 'deferred', updated_at_ms = json_extract(?1, '$.at_ms') WHERE state = 'screening' AND key IN (SELECT value FROM json_each(?1, '$.keys'))";
const FAIL_ALERT: &str = "UPDATE alerts SET reason = json_extract(?1, '$.reason'), updated_at_ms = json_extract(?1, '$.at_ms'), state = CASE WHEN attempts >= json_extract(?1, '$.max_attempts') THEN 'failed' ELSE state END WHERE key = json_extract(?1, '$.key') AND state = json_extract(?1, '$.state')";
const FORGET_THREAD: &str = "UPDATE topics SET thread_id = '', until_ms = 0, thread_at_ms = 0 WHERE id = json_extract(?1, '$.topic_id')";
const FORM: &str = "UPDATE lines SET batch_id = (SELECT COALESCE(MAX(batch_id), 0) + 1 FROM lines), state = json_extract(?1, '$.state'), reviewer = json_extract(?1, '$.reviewer'), updated_at_ms = json_extract(?1, '$.at_ms') WHERE key IN (SELECT key FROM lines WHERE state = 'queued' ORDER BY occurred_at_ms DESC, key DESC LIMIT json_extract(?1, '$.limit')) AND NOT EXISTS (SELECT 1 FROM lines a WHERE a.batch_id > 0 AND NOT EXISTS (SELECT 1 FROM batches b WHERE b.id = a.batch_id AND b.state IN ('sent', 'failed')))";
const NEXT_ALERT: &str = "SELECT json_group_array(json_object('analysis', analysis, 'attempts', attempts, 'key', key, 'probability', probability, 'thread_at_ms', thread_at_ms, 'thread_id', thread_id, 'topic_id', topic_id, 'until_ms', until_ms)) FROM (SELECT a.analysis AS analysis, a.attempts AS attempts, a.key AS key, t.probability AS probability, t.thread_at_ms AS thread_at_ms, t.thread_id AS thread_id, a.topic_id AS topic_id, t.until_ms AS until_ms FROM alerts a JOIN topics t ON t.id = a.topic_id WHERE a.state = json_extract(?1, '$.state') AND a.updated_at_ms <= json_extract(?1, '$.retry_before_ms') AND a.attempts < json_extract(?1, '$.max_attempts') ORDER BY a.created_at_ms, a.key LIMIT 1)";
const OPEN_ALERT: &str = "INSERT OR IGNORE INTO alerts (key, topic_id, state, attempts, analysis, reason, created_at_ms, updated_at_ms) SELECT json_extract(?1, '$.key'), json_extract(?1, '$.topic_id'), 'pending', 0, NULL, '', json_extract(?1, '$.at_ms'), 0 WHERE NOT EXISTS (SELECT 1 FROM alerts WHERE topic_id = json_extract(?1, '$.topic_id') AND created_at_ms > json_extract(?1, '$.cooldown_since_ms')) AND (SELECT COUNT(*) FROM alerts WHERE created_at_ms >= json_extract(?1, '$.hour_since_ms')) < json_extract(?1, '$.max_per_hour')";
const OPEN_LINE: &str = "INSERT OR IGNORE INTO lines (key, conversation_id, occurred_at_ms, sender, text, state, reason, reviewer, flag, hints, rules_rev, batch_id, created_at_ms, updated_at_ms) VALUES (json_extract(?1, '$.key'), json_extract(?1, '$.conversation_id'), json_extract(?1, '$.occurred_at_ms'), json_extract(?1, '$.sender'), json_extract(?1, '$.text'), json_extract(?1, '$.state'), json_extract(?1, '$.reason'), '', '', '[]', json_extract(?1, '$.rules_rev'), 0, json_extract(?1, '$.at_ms'), json_extract(?1, '$.at_ms'))";
const OPEN_TOPICS: &str = "SELECT json_group_array(json_object('label', label, 'topic', id)) FROM (SELECT id, label FROM topics WHERE last_at_ms >= json_extract(?1, '$.since_ms') ORDER BY last_at_ms DESC, id LIMIT json_extract(?1, '$.limit'))";
const PLACE: &str = "INSERT OR IGNORE INTO placements (key, topic_id, probability, batch_id) SELECT json_extract(value, '$.key'), json_extract(value, '$.topic'), json_extract(value, '$.probability'), json_extract(?1, '$.batch_id') FROM json_each(?1, '$.placements')";
const RELEASE_ALERT: &str = "UPDATE alerts SET attempts = attempts - 1, updated_at_ms = 0 WHERE key = json_extract(?1, '$.key') AND state = json_extract(?1, '$.state') AND attempts > 0";
const REQUESTS_SINCE: &str = "SELECT json_group_array(json_object('count', total)) FROM (SELECT COUNT(*) AS total FROM batches WHERE state = 'sent' AND payload <> '' AND updated_at_ms >= json_extract(?1, '$.since_ms'))";
const RESTATE: &str = "UPDATE lines SET text = json_extract(?1, '$.text'), state = CASE WHEN state = 'leaked' THEN 'pending' ELSE state END, rules_rev = json_extract(?1, '$.rev'), updated_at_ms = json_extract(?1, '$.at_ms') WHERE key = json_extract(?1, '$.key')";
const ROLLBACK: &str = "ROLLBACK";
const RULES: &str = "SELECT json_group_array(json_object('id', id, 'pattern', pattern, 'placeholder', placeholder)) FROM (SELECT * FROM learned_rules WHERE id > json_extract(?1, '$.after') ORDER BY id)";
const SCREENED: &str = "UPDATE lines SET text = json_extract(?1, '$.text'), flag = json_extract(?1, '$.flag'), hints = json_extract(?1, '$.hints'), state = json_extract(?1, '$.state'), reason = json_extract(?1, '$.reason'), reviewer = json_extract(?1, '$.reviewer'), updated_at_ms = json_extract(?1, '$.at_ms') WHERE key = json_extract(?1, '$.key') AND state = 'screening'";
const SCREENING: &str = "SELECT json_group_array(json_object('key', key, 'text', text)) FROM (SELECT key, text FROM lines WHERE state = 'screening' AND batch_id > 0 AND (reason = '' OR updated_at_ms <= json_extract(?1, '$.retry_before_ms')) ORDER BY occurred_at_ms, key LIMIT json_extract(?1, '$.limit'))";
const SEALABLE: &str = "SELECT json_group_array(json_object('batch_id', batch_id)) FROM (SELECT DISTINCT l.batch_id AS batch_id FROM lines l WHERE l.batch_id > json_extract(?1, '$.after') AND NOT EXISTS (SELECT 1 FROM batches b WHERE b.id = l.batch_id) AND NOT EXISTS (SELECT 1 FROM lines p WHERE p.batch_id = l.batch_id AND p.state IN ('screening', 'pending', 'leaked')) ORDER BY l.batch_id LIMIT 1)";
const STALE: &str = "SELECT json_group_array(json_object('key', key, 'text', text)) FROM (SELECT key, text FROM lines WHERE state IN ('leaked', 'pending', 'queued', 'screening') AND rules_rev < json_extract(?1, '$.rev') ORDER BY occurred_at_ms, key LIMIT json_extract(?1, '$.limit'))";
const TOPICS: &str = "INSERT INTO topics (id, label, probability, thread_id, until_ms, thread_at_ms, created_at_ms, last_at_ms) SELECT json_extract(value, '$.topic'), json_extract(value, '$.label'), json_extract(value, '$.probability'), '', 0, 0, json_extract(?1, '$.at_ms'), json_extract(value, '$.last_ms') FROM json_each(?1, '$.topics') WHERE true ON CONFLICT(id) DO UPDATE SET probability = NULLIF(MAX(COALESCE(topics.probability, -1), COALESCE(excluded.probability, -1)), -1), last_at_ms = MAX(topics.last_at_ms, excluded.last_at_ms)";
const TOPIC_LINES: &str = "SELECT json_group_array(json_object('occurred_at_ms', occurred_at_ms, 'sender', sender, 'text', text)) FROM (SELECT l.occurred_at_ms AS occurred_at_ms, l.sender AS sender, l.text AS text FROM placements p JOIN lines l ON l.key = p.key WHERE p.topic_id = json_extract(?1, '$.topic_id') AND l.state = 'approved' ORDER BY l.occurred_at_ms, l.key)";
const UNLINED: &str = "SELECT json_group_array(json_object('account', account, 'conversation_id', conversation_id, 'key', key, 'occurred_at_ms', occurred_at_ms, 'sender_id', sender_id, 'sender_name', sender_name, 'source', source, 'text', text)) FROM (SELECT m.account AS account, m.conversation_id AS conversation_id, m.key AS key, m.occurred_at_ms AS occurred_at_ms, m.sender_id AS sender_id, m.sender_name AS sender_name, m.source AS source, m.text AS text FROM messages m WHERE m.occurred_at_ms >= json_extract(?1, '$.since_ms') AND NOT EXISTS (SELECT 1 FROM lines l WHERE l.key = m.key) ORDER BY m.occurred_at_ms DESC, m.key DESC LIMIT json_extract(?1, '$.limit'))";

pub(crate) struct Ledger {
    connection: Connection,
}

impl Ledger {
    pub(crate) fn advance_alert(&self, args: &Value) -> usize {
        self.exec(ADVANCE_ALERT, args)
    }

    pub(crate) fn answer(&self, args: &Value) -> usize {
        self.exec(ANSWER, args)
    }

    pub(crate) fn apply(&self, args: &Value, alerts: &[Value]) -> usize {
        let mut steps = vec![
            Step::new(BEGIN, None),
            Step::new(PLACE, Some(args)),
            Step::new(TOPICS, Some(args)),
        ];
        steps.extend(
            alerts
                .iter()
                .map(|alert| Step::new(OPEN_ALERT, Some(alert))),
        );
        steps.push(Step::new(BATCH_SENT, Some(args)));
        steps.push(Step::new(COMMIT, None));
        self.transact(&steps).unwrap_or_else(|err| {
            tracing::warn!(error = %err, "ledger transaction failed");
            drop(self.connection.execute_batch(ROLLBACK));
            usize::default()
        })
    }

    pub(crate) fn attach(&self, args: &Value) -> Vec<Value> {
        self.rows(ATTACH, args)
    }

    pub(crate) fn batch_lines(&self, args: &Value) -> Vec<Value> {
        self.rows(BATCH_LINES, args)
    }

    pub(crate) fn bind_thread(&self, args: &Value) -> usize {
        self.exec(BIND_THREAD, args)
    }

    pub(crate) fn claim_alert(&self, args: &Value) -> usize {
        self.exec(CLAIM_ALERT, args)
    }

    pub(crate) fn claim_batch(&self, args: &Value) -> usize {
        self.exec(BATCH_CLAIM, args)
    }

    fn count(&self, sql: &str, args: &Value) -> u64 {
        self.rows(sql, args)
            .first()
            .and_then(|row| row.get("count"))
            .and_then(Value::as_u64)
            .unwrap_or_default()
    }

    pub(crate) fn defer(&self, args: &Value) -> usize {
        self.exec(DEFER, args)
    }

    fn exec(&self, sql: &str, args: &Value) -> usize {
        self.connection
            .execute(sql, [args.to_string()])
            .unwrap_or_else(|err| {
                tracing::warn!(error = %err, "ledger write failed");
                0
            })
    }

    pub(crate) fn fail_alert(&self, args: &Value) -> usize {
        self.exec(FAIL_ALERT, args)
    }

    pub(crate) fn fail_batch(&self, args: &Value) -> usize {
        self.exec(BATCH_FAIL, args)
    }

    pub(crate) fn forget_thread(&self, args: &Value) -> usize {
        self.exec(FORGET_THREAD, args)
    }

    pub(crate) fn form(&self, args: &Value) -> usize {
        self.exec(FORM, args)
    }

    pub(crate) fn next_alert(&self, args: &Value) -> Vec<Value> {
        self.rows(NEXT_ALERT, args)
    }

    pub(crate) fn next_batch(&self, args: &Value) -> Vec<Value> {
        self.rows(BATCH_NEXT, args)
    }

    pub(crate) fn open(path: &Path, busy_ms: u64) -> Fallible<Self> {
        Ok(Self {
            connection: connect(path, busy_ms)?,
        })
    }

    pub(crate) fn open_line(&self, args: &Value) -> usize {
        self.exec(OPEN_LINE, args)
    }

    pub(crate) fn open_topics(&self, args: &Value) -> Vec<Value> {
        self.rows(OPEN_TOPICS, args)
    }

    pub(crate) fn release_alert(&self, args: &Value) -> usize {
        self.exec(RELEASE_ALERT, args)
    }

    pub(crate) fn release_batch(&self, args: &Value) -> usize {
        self.exec(BATCH_RELEASE, args)
    }

    pub(crate) fn requests_since(&self, args: &Value) -> u64 {
        self.count(REQUESTS_SINCE, args)
    }

    pub(crate) fn restate(&self, args: &Value) -> usize {
        self.exec(RESTATE, args)
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

    pub(crate) fn rules(&self, args: &Value) -> Vec<Value> {
        self.rows(RULES, args)
    }

    pub(crate) fn screened(&self, args: &Value) -> usize {
        self.exec(SCREENED, args)
    }

    pub(crate) fn screening(&self, args: &Value) -> Vec<Value> {
        self.rows(SCREENING, args)
    }

    pub(crate) fn seal(&self, args: &Value) -> usize {
        self.exec(BATCH_SEAL, args)
    }

    pub(crate) fn sealable(&self, args: &Value) -> Vec<Value> {
        self.rows(SEALABLE, args)
    }

    pub(crate) fn stale(&self, args: &Value) -> Vec<Value> {
        self.rows(STALE, args)
    }

    pub(crate) fn topic_lines(&self, args: &Value) -> Vec<Value> {
        self.rows(TOPIC_LINES, args)
    }

    fn transact(&self, steps: &[Step]) -> Fallible<usize> {
        steps.iter().try_fold(usize::default(), |rows, step| {
            self.connection
                .execute(step.sql, params_from_iter(step.args.iter()))
                .map_err(Box::new)
                .map(|changed| rows.saturating_add(changed))
        })
    }

    pub(crate) fn unlined(&self, args: &Value) -> Vec<Value> {
        self.rows(UNLINED, args)
    }
}

struct Step {
    args: Vec<String>,
    sql: &'static str,
}

impl Step {
    fn new(sql: &'static str, args: Option<&Value>) -> Self {
        Self {
            args: args.map(Value::to_string).into_iter().collect(),
            sql,
        }
    }
}
