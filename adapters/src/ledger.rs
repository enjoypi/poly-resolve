mod sql;

use core::cell::Cell;
use core::iter::once;
use std::path::Path;

use rusqlite::{Connection, Error, params_from_iter};
use serde_json::{Value, from_str};

use crate::store::{Fallible, connect};

pub(crate) struct Ledger {
    connection: Connection,
    faults: Cell<u64>,
}

impl Ledger {
    pub(crate) fn advance_alert(&self, args: &Value) -> usize {
        self.exec(sql::ADVANCE_ALERT, args)
    }

    pub(crate) fn answer(&self, args: &Value) -> usize {
        self.exec(sql::ANSWER, args)
    }

    pub(crate) fn apply(&self, args: &Value, alerts: &[Value]) -> usize {
        let mut steps = vec![
            Step::new(sql::BEGIN, None),
            Step::new(sql::MEMBERS, Some(args)),
            Step::new(sql::UNRELATED, Some(args)),
            Step::new(sql::PLACE_WITHHELD, Some(args)),
            Step::new(sql::RELATE, Some(args)),
            Step::new(sql::VOTE, Some(args)),
            Step::new(sql::UNLIKE, Some(args)),
            Step::new(sql::TOPICS, Some(args)),
            Step::new(sql::REOPEN, Some(args)),
            Step::new(sql::REACH, Some(args)),
            Step::new(sql::RESCORE, Some(args)),
        ];
        steps.extend(alerts.iter().flat_map(|alert| {
            [
                Step::new(sql::MERGE_ALERT, Some(alert)),
                Step::new(sql::OPEN_ALERT, Some(alert)),
            ]
        }));
        steps.push(Step::new(sql::BATCH_SENT, Some(args)));
        steps.push(Step::new(sql::COMMIT, None));
        self.committed(&steps)
    }

    pub(crate) fn archive(&self, args: &Value) -> usize {
        self.exec(sql::ARCHIVE, args)
    }

    pub(crate) fn attach(&self, args: &Value) -> Vec<Value> {
        self.rows(sql::ATTACH, args)
    }

    pub(crate) fn backlog(&self) -> Vec<Value> {
        self.rows(sql::BACKLOG, &Value::Null)
    }

    pub(crate) fn batch_lines(&self, args: &Value) -> Vec<Value> {
        self.rows(sql::BATCH_LINES, args)
    }

    pub(crate) fn changes(&self) -> Vec<Value> {
        self.rows(sql::CHANGES, &Value::Null)
    }

    pub(crate) fn claim_alert(&self, args: &Value) -> usize {
        self.committed(&[
            Step::new(sql::BEGIN, None),
            Step::new(sql::CLAIM_ALERT, Some(args)),
            Step::new(sql::CALL_ALERT, Some(args)),
            Step::new(sql::COMMIT, None),
        ])
    }

    pub(crate) fn claim_batch(&self, args: &Value) -> usize {
        self.committed(&[
            Step::new(sql::BEGIN, None),
            Step::new(sql::BATCH_CLAIM, Some(args)),
            Step::new(sql::CALL_BATCH, Some(args)),
            Step::new(sql::COMMIT, None),
        ])
    }

    fn committed(&self, steps: &[Step]) -> usize {
        self.transact(steps).unwrap_or_else(|err| {
            tracing::warn!(error = %err, "ledger transaction failed");
            self.fault();
            drop(self.connection.execute_batch(sql::ROLLBACK));
            usize::default()
        })
    }

    pub(crate) fn defer(&self, args: &Value) -> usize {
        self.exec(sql::DEFER, args)
    }

    pub(crate) fn drift(&self, args: &Value) -> Vec<Value> {
        self.rows(sql::DRIFT, args)
    }

    fn exec(&self, sql: &str, args: &Value) -> usize {
        self.connection
            .execute(sql, [args.to_string()])
            .unwrap_or_else(|err| {
                tracing::warn!(error = %err, "ledger write failed");
                self.fault();
                0
            })
    }

    pub(crate) fn fail_alert(&self, args: &Value) -> usize {
        self.exec(sql::FAIL_ALERT, args)
    }

    pub(crate) fn fail_batch(&self, args: &Value) -> usize {
        self.exec(sql::BATCH_FAIL, args)
    }

    fn fault(&self) {
        self.faults.set(self.faults.get().saturating_add(1));
    }

    pub(crate) const fn faults(&self) -> u64 {
        self.faults.get()
    }

    pub(crate) fn forget_thread(&self, args: &Value) -> usize {
        self.exec(sql::FORGET_THREAD, args)
    }

    pub(crate) fn form(&self, args: &Value) -> usize {
        self.exec(sql::FORM, args)
    }

    pub(crate) fn identify(&self, args: &Value) -> usize {
        self.exec(sql::IDENTIFY, args)
    }

    pub(crate) fn index(&self, args: &Value) -> usize {
        self.exec(sql::CLUES, args)
    }

    pub(crate) fn intake_since(&self, args: &Value) -> Option<u64> {
        let _written = self.exec(sql::INTAKE_STATE, args);
        self.rows(sql::INTAKE_SINCE, args)
            .first()
            .and_then(|row| row.get("since_ms"))
            .and_then(Value::as_u64)
    }

    pub(crate) fn linked(&self, args: &Value) -> Vec<Value> {
        self.rows(sql::LINKED, args)
    }

    pub(crate) fn next_alert(&self, args: &Value) -> Vec<Value> {
        self.rows(sql::NEXT_ALERT, args)
    }

    pub(crate) fn next_batch(&self, args: &Value) -> Vec<Value> {
        self.rows(sql::BATCH_NEXT, args)
    }

    pub(crate) fn open(path: &Path, busy_ms: u64) -> Fallible<Self> {
        let ledger = Self {
            connection: connect(path, busy_ms)?,
            faults: Cell::new(0),
        };
        ledger
            .connection
            .execute_batch(sql::MIGRATE)
            .map_err(Box::new)?;
        Ok(ledger)
    }

    pub(crate) fn open_line(&self, args: &Value) -> usize {
        self.exec(sql::OPEN_LINE, args)
    }

    pub(crate) fn open_topics(&self, args: &Value) -> Vec<Value> {
        self.rows(sql::OPEN_TOPICS, args)
    }

    pub(crate) fn propose(&self, args: &Value) -> usize {
        self.exec(sql::PROPOSE, args)
    }

    pub(crate) fn record_analysis(&self, args: &Value) -> usize {
        self.committed(
            &vec![
                Step::new(sql::BEGIN, None),
                Step::new(sql::RENAME_TOPIC, Some(args)),
                Step::new(sql::RENAME_MEMBERS, Some(args)),
                Step::new(sql::RENAME_ALERTS, Some(args)),
                Step::new(sql::RENAME_CALLS, Some(args)),
                Step::new(sql::RENAME_CHANGES_FROM, Some(args)),
                Step::new(sql::RENAME_CHANGES_TO, Some(args)),
                Step::new(sql::RENAME_RELATIONS_A, Some(args)),
                Step::new(sql::RENAME_RELATIONS_B, Some(args)),
                Step::new(sql::RENAME_RELATIONS_DROP, Some(args)),
                Step::new(sql::ALIAS_CHAIN, Some(args)),
                Step::new(sql::ALIAS_TOPIC, Some(args)),
                Step::new(sql::RENAME_DELETE, Some(args)),
                Step::new(sql::BIND_THREAD, Some(args)),
                Step::new(sql::VERDICT, Some(args)),
                Step::new(sql::RECORD_ANALYSIS, Some(args)),
                Step::new(sql::COMMIT, None),
            ]
            .into_boxed_slice(),
        )
    }

    pub(crate) fn release_alert(&self, args: &Value) -> usize {
        self.committed(&[
            Step::new(sql::BEGIN, None),
            Step::new(sql::RELEASE_CALL_ALERT, Some(args)),
            Step::new(sql::RELEASE_ALERT, Some(args)),
            Step::new(sql::COMMIT, None),
        ])
    }

    pub(crate) fn release_batch(&self, args: &Value) -> usize {
        self.committed(&[
            Step::new(sql::BEGIN, None),
            Step::new(sql::BATCH_CALL_RELEASE, Some(args)),
            Step::new(sql::BATCH_RELEASE, Some(args)),
            Step::new(sql::COMMIT, None),
        ])
    }

    pub(crate) fn restate(&self, args: &Value) -> usize {
        self.exec(sql::RESTATE, args)
    }

    pub(crate) fn reviews(&self, args: &Value) -> Vec<Value> {
        self.rows(sql::REVIEWS, args)
    }

    pub(crate) fn revive(&self, args: &Value) -> usize {
        self.exec(sql::REVIVE, args)
    }

    fn rows(&self, sql: &str, args: &Value) -> Vec<Value> {
        let found: Result<String, Box<Error>> =
            self.connection
                .query_row_and_then(sql, [args.to_string()], |row| row.get(0).map_err(Box::new));
        found.map_or_else(
            |err| {
                tracing::warn!(error = %err, "ledger read failed");
                self.fault();
                Vec::new()
            },
            |json| from_str(&json).unwrap_or_default(),
        )
    }

    pub(crate) fn rules(&self, args: &Value) -> Vec<Value> {
        self.rows(sql::RULES, args)
    }

    pub(crate) fn screened(&self, args: &Value) -> usize {
        self.committed(&[
            Step::new(sql::BEGIN, None),
            Step::new(sql::MISS, Some(args)),
            Step::new(sql::SCREENED, Some(args)),
            Step::new(sql::COMMIT, None),
        ])
    }

    pub(crate) fn screening(&self, args: &Value) -> Vec<Value> {
        self.rows(sql::SCREENING, args)
    }

    pub(crate) fn seal(&self, args: &Value) -> usize {
        self.exec(sql::BATCH_SEAL, args)
    }

    pub(crate) fn sealable(&self, args: &Value) -> Vec<Value> {
        self.rows(sql::SEALABLE, args)
    }

    pub(crate) fn settle(&self, args: &Value) -> usize {
        let steps: Vec<Step> = once(Step::new(sql::BEGIN, None))
            .chain(
                sql::SETTLE
                    .iter()
                    .map(|statement| Step::new(statement, Some(args))),
            )
            .chain(once(Step::new(sql::COMMIT, None)))
            .collect();
        self.committed(&steps)
    }

    pub(crate) fn stale(&self, args: &Value) -> Vec<Value> {
        self.rows(sql::STALE, args)
    }

    pub(crate) fn strangers(&self) -> Vec<Value> {
        self.rows(sql::STRANGERS, &Value::Null)
    }

    pub(crate) fn stuck(&self, args: &Value) -> Vec<Value> {
        self.rows(sql::STUCK, args)
    }

    pub(crate) fn supersede_notices(&self) -> usize {
        self.exec(sql::NOTICE_SUPERSEDE, &Value::Null)
    }

    pub(crate) fn superseded(&self, args: &Value) -> bool {
        !self.rows(sql::SUPERSEDED, args).is_empty()
    }

    pub(crate) fn topic_lines(&self, args: &Value) -> Vec<Value> {
        self.rows(sql::TOPIC_LINES, args)
    }

    fn transact(&self, steps: &[Step]) -> Fallible<usize> {
        steps.iter().try_fold(usize::default(), |rows, step| {
            self.connection
                .execute(step.sql, params_from_iter(step.args.iter()))
                .map_err(Box::new)
                .map(|changed| rows.saturating_add(step.counted(changed)))
        })
    }

    pub(crate) fn unindexed(&self, args: &Value) -> Vec<Value> {
        self.rows(sql::UNINDEXED, args)
    }

    pub(crate) fn unlined(&self, args: &Value) -> Vec<Value> {
        self.rows(sql::UNLINED, args)
    }

    pub(crate) fn wake_alerts(&self) -> usize {
        self.exec(sql::WAKE_ALERTS, &Value::Null)
    }
}

struct Step {
    args: Vec<String>,
    sql: &'static str,
}

impl Step {
    const fn counted(&self, changed: usize) -> usize {
        if self.args.is_empty() { 0 } else { changed }
    }

    fn new(sql: &'static str, args: Option<&Value>) -> Self {
        Self {
            args: args.map(Value::to_string).into_iter().collect(),
            sql,
        }
    }
}
