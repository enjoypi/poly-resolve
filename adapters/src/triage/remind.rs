use core::pin::pin;
use std::time::Instant;

use futures_util::future::join_all;
use serde_json::{Value, json};

use super::{FAILED, SENT, SUCCESS, Shift, TRIAGE, pace, text};
use crate::ingest::now_ms;
use crate::pipe::{Job, feed};
use crate::race::race;
use crate::trace::{Outcome, external, finished};

struct Reminder {
    extra: Vec<String>,
    key: String,
    notice: String,
    started: Instant,
}

fn jobs(shift: &Shift<'_>, due: &[Value]) -> Vec<Reminder> {
    due.iter()
        .filter_map(|row| {
            let key = text(row, "key").to_owned();
            let claim = json!({
                "at_ms": now_ms(),
                "key": key,
                "max_attempts": shift.triage.max_attempts(),
            });
            if shift.ledger.claim_reminder(&claim) == 0 {
                return None;
            }
            Some(Reminder {
                extra: vec![
                    shift.triage.notify_target().value().unwrap_or_default(),
                    format!("--idempotency-key={key}"),
                ],
                key,
                notice: shift.desk.remind(row),
                started: Instant::now(),
            })
        })
        .collect()
}

pub(super) async fn remind(shift: &mut Shift<'_>) -> Option<()> {
    let triage = shift.triage;
    let due = shift.ledger.reminders(&json!({
        "daily_tokens": triage.daily_tokens(),
        "down_after_ms": triage.down_after_ms(),
        "max_attempts": triage.max_attempts(),
        "pace": pace(shift),
        "started_ms": shift.started_ms,
    }));
    let reminders = jobs(shift, &due);
    if reminders.is_empty() {
        return Some(());
    }
    let work = join_all(
        reminders
            .iter()
            .map(|job| feed(Job::new(triage.notify(), &job.extra, &job.notice))),
    );
    let Some(outcomes) = race(&mut *shift.stop, pin!(work)).await else {
        for job in &reminders {
            let released = shift.ledger.release_reminder(&json!({"key": job.key}));
            tracing::debug!(released, "reminder claim released");
        }
        return None;
    };
    for (job, done) in reminders.iter().zip(outcomes) {
        let status = finished(&done);
        let elapsed = u64::try_from(job.started.elapsed().as_micros()).unwrap_or(u64::MAX);
        external(
            "remind",
            job.notice.len(),
            &Outcome::new(0, status.clone(), elapsed).within(TRIAGE),
        );
        let state = if status == SUCCESS { SENT } else { FAILED };
        let rows = shift
            .ledger
            .settle_reminder(&json!({"at_ms": now_ms(), "key": job.key, "state": state}));
        tracing::info!(rows, state, "reminder settled");
    }
    Some(())
}
