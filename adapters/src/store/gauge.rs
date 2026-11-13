use domain::AppError;
use domain::gauge::{Day, Health, Meter};
use rusqlite::{OpenFlags, params};
use serde_json::from_str;

use super::{Failure, Shelf};

const SOURCES: &str = "SELECT json_object('sources', json((SELECT json_group_array(json_object('account', h.account, 'down_at_ms', h.down_at_ms, 'last_at_ms', (SELECT MAX(m.occurred_at_ms) FROM messages m WHERE m.source = h.source AND m.account = h.account), 'kind', h.source, 'ok_at_ms', h.ok_at_ms, 'state', h.state, 'streak', h.streak)) FROM (SELECT * FROM source_health ORDER BY source, account) h)), 'stages', json((SELECT json_group_array(json_object('kind', k.kind, 'streak', (SELECT COUNT(*) FROM calls c WHERE c.kind = k.kind AND c.state = 'failed' AND c.at_ms > COALESCE((SELECT MAX(s.at_ms) FROM calls s WHERE s.kind = k.kind AND s.state = 'success'), 0)))) FROM (SELECT 'analysis' AS kind UNION ALL SELECT 'decision' UNION ALL SELECT 'notify' UNION ALL SELECT 'screen') k)))";
const USAGE: &str = "SELECT json_group_array(json_object('calls', total, 'date', day, 'failures', failures, 'input_tokens', input_tokens, 'kind', kind, 'output_tokens', output_tokens)) FROM (SELECT date(at_ms / 1000, 'unixepoch') AS day, kind, COUNT(*) AS total, SUM(state = 'failed') AS failures, SUM(input_tokens) AS input_tokens, SUM(output_tokens) AS output_tokens FROM calls WHERE date(at_ms / 1000, 'unixepoch') > date('now', '-' || ?1 || ' days') GROUP BY day, kind ORDER BY day DESC, kind)";

impl Shelf<'_> {
    fn health(&self) -> Result<Health, Failure> {
        let connection = self.open(OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let json: String =
            connection.query_row_and_then(SOURCES, [], |row| row.get(0).map_err(Box::new))?;
        Ok(from_str(&json)?)
    }

    fn spent(&self, days: u32) -> Result<Vec<Day>, Failure> {
        let connection = self.open(OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let json: String = connection
            .query_row_and_then(USAGE, params![days], |row| row.get(0).map_err(Box::new))?;
        Ok(from_str(&json)?)
    }
}

impl Meter for Shelf<'_> {
    fn sources(&self) -> Result<Health, AppError> {
        self.health().map_err(|err| {
            tracing::warn!(error = %err, "health read failed");
            AppError::Storage
        })
    }

    fn usage(&self, days: u32) -> Result<Vec<Day>, AppError> {
        self.spent(days).map_err(|err| {
            tracing::warn!(error = %err, "usage read failed");
            AppError::Storage
        })
    }
}
