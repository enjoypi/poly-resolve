use std::path::Path;

use rusqlite::{Connection, Error, params_from_iter};
use serde_json::{Value, from_str};

use crate::store::{Fallible, connect};

const ADVANCE_ALERT: &str = "UPDATE alerts SET state = json_extract(?1, '$.state'), analysis = COALESCE(json_extract(?1, '$.analysis'), analysis), reason = '', attempts = 0, updated_at_ms = 0 WHERE key = json_extract(?1, '$.key') AND state = json_extract(?1, '$.from')";
const ANSWER: &str = "INSERT OR REPLACE INTO answers (batch_id, body) VALUES (json_extract(?1, '$.batch_id'), json_extract(?1, '$.body'))";
const ATTACH: &str = "SELECT json_group_array(json_object('conversation_id', conversation_id, 'last_ms', last_ms, 'probability', probability, 'topic_id', topic_id)) FROM (SELECT l.conversation_id AS conversation_id, MAX(l.occurred_at_ms) AS last_ms, MAX(t.probability) AS probability, p.topic_id AS topic_id FROM placements p JOIN lines l ON l.key = p.key JOIN topics t ON t.id = p.topic_id WHERE t.last_at_ms >= json_extract(?1, '$.since_ms') AND l.conversation_id IN (SELECT conversation_id FROM lines WHERE batch_id = json_extract(?1, '$.batch_id')) GROUP BY l.conversation_id)";
const BATCH_CALL_RELEASE: &str = "DELETE FROM calls WHERE kind = 'decision' AND subject = CAST(json_extract(?1, '$.id') AS TEXT) AND attempt = (SELECT attempts FROM batches WHERE id = json_extract(?1, '$.id'))";
const BATCH_CLAIM: &str = "UPDATE batches SET attempts = attempts + 1, updated_at_ms = json_extract(?1, '$.at_ms') WHERE id = json_extract(?1, '$.id') AND state = 'ready' AND attempts < json_extract(?1, '$.max_attempts')";
const BATCH_FAIL: &str = "UPDATE batches SET reason = json_extract(?1, '$.reason'), updated_at_ms = json_extract(?1, '$.at_ms'), state = CASE WHEN attempts >= json_extract(?1, '$.max_attempts') THEN 'failed' ELSE state END WHERE id = json_extract(?1, '$.id') AND state = 'ready'";
const BATCH_LINES: &str = "SELECT json_group_array(json_object('conversation_id', conversation_id, 'hints', json(hints), 'key', key, 'occurred_at_ms', occurred_at_ms, 'sender', sender, 'text', text)) FROM (SELECT * FROM lines WHERE batch_id = json_extract(?1, '$.batch_id') AND state = 'approved' ORDER BY occurred_at_ms, key)";
const BATCH_NEXT: &str = "SELECT json_group_array(json_object('answer', answer, 'attempts', attempts, 'id', id, 'layout', layout, 'payload', payload)) FROM (SELECT a.body AS answer, b.attempts AS attempts, b.id AS id, b.layout AS layout, b.payload AS payload FROM batches b LEFT JOIN answers a ON a.batch_id = b.id WHERE b.state = 'ready' AND b.updated_at_ms <= json_extract(?1, '$.retry_before_ms') AND (a.body IS NOT NULL OR b.attempts < json_extract(?1, '$.max_attempts')) ORDER BY b.id LIMIT 1)";
const BATCH_RELEASE: &str = "UPDATE batches SET attempts = attempts - 1, updated_at_ms = 0 WHERE id = json_extract(?1, '$.id') AND state = 'ready' AND attempts > 0";
const BATCH_SEAL: &str = "INSERT OR IGNORE INTO batches (id, state, payload, layout, attempts, reason, created_at_ms, updated_at_ms) VALUES (json_extract(?1, '$.id'), json_extract(?1, '$.state'), json_extract(?1, '$.payload'), json_extract(?1, '$.layout'), 0, '', json_extract(?1, '$.at_ms'), json_extract(?1, '$.updated_ms'))";
const BATCH_SENT: &str = "UPDATE batches SET state = 'sent', attempts = 0, reason = '', updated_at_ms = json_extract(?1, '$.at_ms') WHERE id = json_extract(?1, '$.batch_id') AND state = 'ready'";
const BEGIN: &str = "BEGIN IMMEDIATE";
const BIND_THREAD: &str = "UPDATE topics SET thread_id = json_extract(?1, '$.thread_id'), until_ms = json_extract(?1, '$.until_ms'), until_key = json_extract(?1, '$.until_key'), thread_at_ms = CASE WHEN thread_at_ms = 0 THEN json_extract(?1, '$.at_ms') ELSE thread_at_ms END WHERE id = json_extract(?1, '$.thread_id')";
const CALL_ALERT: &str = "INSERT INTO calls (kind, subject, topic_id, attempt, at_ms) SELECT json_extract(?1, '$.kind'), key, topic_id, attempts, json_extract(?1, '$.at_ms') FROM alerts WHERE key = json_extract(?1, '$.key') AND updated_at_ms = json_extract(?1, '$.at_ms')";
const CALL_BATCH: &str = "INSERT INTO calls (kind, subject, topic_id, attempt, at_ms) SELECT 'decision', CAST(id AS TEXT), '', attempts, json_extract(?1, '$.at_ms') FROM batches WHERE id = json_extract(?1, '$.id') AND updated_at_ms = json_extract(?1, '$.at_ms')";
const CLAIM_ALERT: &str = "UPDATE alerts SET attempts = attempts + 1, updated_at_ms = json_extract(?1, '$.at_ms') WHERE key = json_extract(?1, '$.key') AND state = json_extract(?1, '$.state') AND attempts < json_extract(?1, '$.max_attempts') AND (state <> 'pending' OR analysis IS NOT NULL OR ((SELECT COUNT(*) FROM calls c WHERE c.kind = 'analysis' AND c.at_ms >= json_extract(?1, '$.hour_since_ms')) < json_extract(?1, '$.max_per_hour') AND NOT EXISTS (SELECT 1 FROM calls c WHERE c.kind = 'analysis' AND c.topic_id = alerts.topic_id AND c.at_ms > json_extract(?1, '$.cooldown_since_ms'))))";
const COMMIT: &str = "COMMIT";
const DEFER: &str = "UPDATE lines SET reason = 'deferred', updated_at_ms = json_extract(?1, '$.at_ms') WHERE state = 'screening' AND key IN (SELECT value FROM json_each(?1, '$.keys'))";
const FAIL_ALERT: &str = "UPDATE alerts SET reason = json_extract(?1, '$.reason'), updated_at_ms = json_extract(?1, '$.at_ms'), state = CASE WHEN attempts >= json_extract(?1, '$.max_attempts') THEN 'failed' ELSE state END WHERE key = json_extract(?1, '$.key') AND state = json_extract(?1, '$.state')";
const FORGET_THREAD: &str =
    "UPDATE topics SET thread_at_ms = 0 WHERE id = json_extract(?1, '$.topic_id')";
const FORM: &str = "UPDATE lines SET batch_id = (SELECT COALESCE(MAX(batch_id), 0) + 1 FROM lines), state = json_extract(?1, '$.state'), reviewer = json_extract(?1, '$.reviewer'), updated_at_ms = json_extract(?1, '$.at_ms') WHERE key IN (SELECT key FROM lines WHERE state = 'queued' ORDER BY occurred_at_ms DESC, key DESC LIMIT json_extract(?1, '$.limit')) AND NOT EXISTS (SELECT 1 FROM lines a WHERE a.batch_id > 0 AND NOT EXISTS (SELECT 1 FROM batches b WHERE b.id = a.batch_id AND b.state IN ('sent', 'failed')))";
const INTAKE_SINCE: &str = "SELECT json_group_array(json_object('since_ms', since_ms)) FROM triage_state WHERE id = 1 AND json_extract(?1, '$.enabled') IS NOT NULL";
const INTAKE_STATE: &str = "INSERT INTO triage_state (id, enabled, since_ms) VALUES (1, json_extract(?1, '$.enabled'), json_extract(?1, '$.since_ms')) ON CONFLICT(id) DO UPDATE SET since_ms = CASE WHEN triage_state.enabled = 0 AND excluded.enabled = 1 THEN excluded.since_ms ELSE triage_state.since_ms END, enabled = excluded.enabled";
const MIGRATE: &str = "BEGIN IMMEDIATE; INSERT INTO topics (id, label, probability, thread_id, until_ms, until_key, thread_at_ms, created_at_ms, last_at_ms) SELECT thread_id, label, probability, thread_id, until_ms, until_key, thread_at_ms, created_at_ms, last_at_ms FROM topics WHERE id <> thread_id AND thread_id <> ''; UPDATE placements SET topic_id = (SELECT t.thread_id FROM topics t WHERE t.id = placements.topic_id) WHERE EXISTS (SELECT 1 FROM topics t WHERE t.id = placements.topic_id AND t.id <> t.thread_id AND t.thread_id <> ''); UPDATE alerts SET topic_id = (SELECT t.thread_id FROM topics t WHERE t.id = alerts.topic_id) WHERE EXISTS (SELECT 1 FROM topics t WHERE t.id = alerts.topic_id AND t.id <> t.thread_id AND t.thread_id <> ''); UPDATE calls SET topic_id = (SELECT t.thread_id FROM topics t WHERE t.id = calls.topic_id) WHERE EXISTS (SELECT 1 FROM topics t WHERE t.id = calls.topic_id AND t.id <> t.thread_id AND t.thread_id <> ''); DELETE FROM topics WHERE id <> thread_id AND thread_id <> ''; COMMIT;";
const NEXT_ALERT: &str = "SELECT json_group_array(json_object('analysis', analysis, 'attempts', attempts, 'key', key, 'probability', probability, 'thread_at_ms', thread_at_ms, 'thread_id', thread_id, 'topic_id', topic_id, 'until_key', until_key, 'until_ms', until_ms)) FROM (SELECT a.analysis AS analysis, a.attempts AS attempts, a.key AS key, t.probability AS probability, t.thread_at_ms AS thread_at_ms, t.thread_id AS thread_id, a.topic_id AS topic_id, t.until_key AS until_key, t.until_ms AS until_ms FROM alerts a JOIN topics t ON t.id = a.topic_id WHERE a.state = json_extract(?1, '$.state') AND a.updated_at_ms <= json_extract(?1, '$.retry_before_ms') AND (a.attempts < json_extract(?1, '$.max_attempts') OR (a.state = 'pending' AND a.analysis IS NOT NULL)) AND (a.state <> 'pending' OR a.analysis IS NOT NULL OR ((SELECT COUNT(*) FROM calls c WHERE c.kind = 'analysis' AND c.at_ms >= json_extract(?1, '$.hour_since_ms')) < json_extract(?1, '$.max_per_hour') AND NOT EXISTS (SELECT 1 FROM calls c WHERE c.kind = 'analysis' AND c.topic_id = a.topic_id AND c.at_ms > json_extract(?1, '$.cooldown_since_ms')))) ORDER BY a.created_at_ms, a.key LIMIT json_extract(?1, '$.parallel'))";
const OPEN_ALERT: &str = "INSERT OR IGNORE INTO alerts (key, topic_id, state, attempts, analysis, reason, created_at_ms, updated_at_ms) SELECT json_extract(?1, '$.key'), json_extract(?1, '$.topic_id'), 'waiting', 0, NULL, '', json_extract(?1, '$.at_ms'), 0 WHERE NOT EXISTS (SELECT 1 FROM alerts WHERE topic_id = json_extract(?1, '$.topic_id') AND state = 'waiting')";
const OPEN_LINE: &str = "INSERT OR IGNORE INTO lines (key, conversation_id, occurred_at_ms, sender, text, state, reason, reviewer, flag, hints, rules_rev, batch_id, created_at_ms, updated_at_ms) VALUES (json_extract(?1, '$.key'), json_extract(?1, '$.conversation_id'), json_extract(?1, '$.occurred_at_ms'), json_extract(?1, '$.sender'), json_extract(?1, '$.text'), json_extract(?1, '$.state'), json_extract(?1, '$.reason'), '', '', '[]', json_extract(?1, '$.rules_rev'), 0, json_extract(?1, '$.at_ms'), json_extract(?1, '$.at_ms'))";
const OPEN_TOPICS: &str = "SELECT json_group_array(json_object('label', label, 'probability', probability, 'topic', id)) FROM (SELECT id, label, probability FROM topics WHERE last_at_ms >= json_extract(?1, '$.since_ms') ORDER BY last_at_ms DESC, id LIMIT json_extract(?1, '$.limit'))";
const PLACE: &str = "INSERT OR IGNORE INTO placements (key, topic_id, probability, batch_id) SELECT json_extract(value, '$.key'), json_extract(value, '$.topic'), json_extract(value, '$.probability'), json_extract(?1, '$.batch_id') FROM json_each(?1, '$.placements')";
const RECORD_ANALYSIS: &str = "UPDATE alerts SET analysis = json_extract(?1, '$.analysis'), reason = '', updated_at_ms = 0 WHERE key = json_extract(?1, '$.key') AND state = 'pending'";
const RELEASE_ALERT: &str = "UPDATE alerts SET attempts = attempts - 1, updated_at_ms = 0 WHERE key = json_extract(?1, '$.key') AND state = json_extract(?1, '$.state') AND attempts > 0";
const RELEASE_CALL_ALERT: &str = "DELETE FROM calls WHERE kind = json_extract(?1, '$.kind') AND subject = json_extract(?1, '$.key') AND attempt = (SELECT attempts FROM alerts WHERE key = json_extract(?1, '$.key'))";
const RENAME_ALERTS: &str = "UPDATE alerts SET topic_id = json_extract(?1, '$.thread_id') WHERE topic_id = json_extract(?1, '$.topic_id')";
const RENAME_CALLS: &str = "UPDATE calls SET topic_id = json_extract(?1, '$.thread_id') WHERE topic_id = json_extract(?1, '$.topic_id')";
const RENAME_DELETE: &str = "DELETE FROM topics WHERE id = json_extract(?1, '$.topic_id') AND id <> json_extract(?1, '$.thread_id')";
const RENAME_PLACEMENTS: &str = "UPDATE placements SET topic_id = json_extract(?1, '$.thread_id') WHERE topic_id = json_extract(?1, '$.topic_id')";
const RENAME_TOPIC: &str = "INSERT INTO topics (id, label, probability, thread_id, until_ms, until_key, thread_at_ms, created_at_ms, last_at_ms) SELECT json_extract(?1, '$.thread_id'), label, probability, thread_id, until_ms, until_key, thread_at_ms, created_at_ms, last_at_ms FROM topics WHERE id = json_extract(?1, '$.topic_id') AND id <> json_extract(?1, '$.thread_id')";
const REQUESTS_SINCE: &str = "SELECT json_group_array(json_object('count', total)) FROM (SELECT COUNT(*) AS total FROM calls WHERE kind = 'decision' AND at_ms >= json_extract(?1, '$.since_ms'))";
const RESTATE: &str = "UPDATE lines SET text = json_extract(?1, '$.text'), state = CASE WHEN state = 'leaked' THEN 'pending' ELSE state END, rules_rev = json_extract(?1, '$.rev'), updated_at_ms = json_extract(?1, '$.at_ms') WHERE key = json_extract(?1, '$.key')";
const ROLLBACK: &str = "ROLLBACK";
const RULES: &str = "SELECT json_group_array(json_object('id', id, 'pattern', pattern, 'placeholder', placeholder)) FROM (SELECT * FROM learned_rules WHERE id > json_extract(?1, '$.after') ORDER BY id)";
const SCREENED: &str = "UPDATE lines SET text = json_extract(?1, '$.text'), flag = json_extract(?1, '$.flag'), hints = json_extract(?1, '$.hints'), state = json_extract(?1, '$.state'), reason = json_extract(?1, '$.reason'), reviewer = json_extract(?1, '$.reviewer'), updated_at_ms = json_extract(?1, '$.at_ms') WHERE key = json_extract(?1, '$.key') AND state = 'screening'";
const SCREENING: &str = "SELECT json_group_array(json_object('key', key, 'text', text)) FROM (SELECT key, text FROM lines WHERE state = 'screening' AND batch_id > 0 AND (reason = '' OR updated_at_ms <= json_extract(?1, '$.retry_before_ms')) ORDER BY occurred_at_ms, key LIMIT json_extract(?1, '$.limit'))";
const SEALABLE: &str = "SELECT json_group_array(json_object('batch_id', batch_id)) FROM (SELECT DISTINCT l.batch_id AS batch_id FROM lines l WHERE l.batch_id > json_extract(?1, '$.after') AND NOT EXISTS (SELECT 1 FROM batches b WHERE b.id = l.batch_id) AND NOT EXISTS (SELECT 1 FROM lines p WHERE p.batch_id = l.batch_id AND p.state IN ('screening', 'pending', 'leaked')) ORDER BY l.batch_id LIMIT 1)";
const STALE: &str = "SELECT json_group_array(json_object('key', key, 'text', text)) FROM (SELECT key, text FROM lines WHERE state IN ('leaked', 'pending', 'queued', 'screening') AND rules_rev < json_extract(?1, '$.rev') ORDER BY occurred_at_ms, key LIMIT json_extract(?1, '$.limit'))";
const TOPICS: &str = "INSERT INTO topics (id, label, probability, thread_id, until_ms, until_key, thread_at_ms, created_at_ms, last_at_ms) SELECT json_extract(value, '$.topic'), json_extract(value, '$.label'), json_extract(value, '$.probability'), '', 0, '', 0, json_extract(?1, '$.at_ms'), json_extract(value, '$.last_ms') FROM json_each(?1, '$.topics') WHERE true ON CONFLICT(id) DO UPDATE SET probability = NULLIF(MAX(COALESCE(topics.probability, -1), COALESCE(excluded.probability, -1)), -1), last_at_ms = MAX(topics.last_at_ms, excluded.last_at_ms)";
const TOPIC_LINES: &str = "SELECT json_group_array(json_object('key', key, 'occurred_at_ms', occurred_at_ms, 'sender', sender, 'text', text)) FROM (SELECT l.key AS key, l.occurred_at_ms AS occurred_at_ms, l.sender AS sender, l.text AS text FROM placements p JOIN lines l ON l.key = p.key WHERE p.topic_id = json_extract(?1, '$.topic_id') AND l.state = 'approved' ORDER BY l.occurred_at_ms, l.key)";
const UNLINED: &str = "SELECT json_group_array(json_object('account', account, 'conversation_id', conversation_id, 'key', key, 'occurred_at_ms', occurred_at_ms, 'sender_id', sender_id, 'sender_name', sender_name, 'source', source, 'text', text)) FROM (SELECT m.account AS account, m.conversation_id AS conversation_id, m.key AS key, m.occurred_at_ms AS occurred_at_ms, m.sender_id AS sender_id, m.sender_name AS sender_name, m.source AS source, m.text AS text FROM messages m WHERE m.occurred_at_ms >= json_extract(?1, '$.since_ms') AND NOT EXISTS (SELECT 1 FROM lines l WHERE l.key = m.key) ORDER BY m.occurred_at_ms DESC, m.key DESC LIMIT json_extract(?1, '$.limit'))";

const WAKE_ALERTS: &str = "UPDATE alerts SET state = 'pending', updated_at_ms = 0 WHERE state = 'waiting' AND json_extract(?1, '$') IS NULL";

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
        self.committed(&steps)
    }

    pub(crate) fn attach(&self, args: &Value) -> Vec<Value> {
        self.rows(ATTACH, args)
    }

    pub(crate) fn batch_lines(&self, args: &Value) -> Vec<Value> {
        self.rows(BATCH_LINES, args)
    }

    pub(crate) fn claim_alert(&self, args: &Value) -> usize {
        self.committed(&[
            Step::new(BEGIN, None),
            Step::new(CLAIM_ALERT, Some(args)),
            Step::new(CALL_ALERT, Some(args)),
            Step::new(COMMIT, None),
        ])
    }

    pub(crate) fn claim_batch(&self, args: &Value) -> usize {
        self.committed(&[
            Step::new(BEGIN, None),
            Step::new(BATCH_CLAIM, Some(args)),
            Step::new(CALL_BATCH, Some(args)),
            Step::new(COMMIT, None),
        ])
    }

    fn committed(&self, steps: &[Step]) -> usize {
        self.transact(steps).unwrap_or_else(|err| {
            tracing::warn!(error = %err, "ledger transaction failed");
            drop(self.connection.execute_batch(ROLLBACK));
            usize::default()
        })
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

    pub(crate) fn intake_since(&self, args: &Value) -> Option<u64> {
        let _written = self.exec(INTAKE_STATE, args);
        self.rows(INTAKE_SINCE, args)
            .first()
            .and_then(|row| row.get("since_ms"))
            .and_then(Value::as_u64)
    }

    pub(crate) fn next_alert(&self, args: &Value) -> Vec<Value> {
        self.rows(NEXT_ALERT, args)
    }

    pub(crate) fn next_batch(&self, args: &Value) -> Vec<Value> {
        self.rows(BATCH_NEXT, args)
    }

    pub(crate) fn open(path: &Path, busy_ms: u64) -> Fallible<Self> {
        let ledger = Self {
            connection: connect(path, busy_ms)?,
        };
        ledger.connection.execute_batch(MIGRATE).map_err(Box::new)?;
        Ok(ledger)
    }

    pub(crate) fn open_line(&self, args: &Value) -> usize {
        self.exec(OPEN_LINE, args)
    }

    pub(crate) fn open_topics(&self, args: &Value) -> Vec<Value> {
        self.rows(OPEN_TOPICS, args)
    }

    pub(crate) fn record_analysis(&self, args: &Value) -> usize {
        self.committed(
            &vec![
                Step::new(BEGIN, None),
                Step::new(RENAME_TOPIC, Some(args)),
                Step::new(RENAME_PLACEMENTS, Some(args)),
                Step::new(RENAME_ALERTS, Some(args)),
                Step::new(RENAME_CALLS, Some(args)),
                Step::new(RENAME_DELETE, Some(args)),
                Step::new(BIND_THREAD, Some(args)),
                Step::new(RECORD_ANALYSIS, Some(args)),
                Step::new(COMMIT, None),
            ]
            .into_boxed_slice(),
        )
    }

    pub(crate) fn release_alert(&self, args: &Value) -> usize {
        self.committed(&[
            Step::new(BEGIN, None),
            Step::new(RELEASE_CALL_ALERT, Some(args)),
            Step::new(RELEASE_ALERT, Some(args)),
            Step::new(COMMIT, None),
        ])
    }

    pub(crate) fn release_batch(&self, args: &Value) -> usize {
        self.committed(&[
            Step::new(BEGIN, None),
            Step::new(BATCH_CALL_RELEASE, Some(args)),
            Step::new(BATCH_RELEASE, Some(args)),
            Step::new(COMMIT, None),
        ])
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

    pub(crate) fn wake_alerts(&self) -> usize {
        self.exec(WAKE_ALERTS, &Value::Null)
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
