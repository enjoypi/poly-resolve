use core::pin::pin;
use std::io;
use std::process::Output;
use std::time::Instant;

use futures_util::future::join_all;
use serde_json::{Value, json};

use super::desk::Brief;
use super::trace::{Outcome, external};
use super::{ANALYZED, HOUR_MS, PENDING, SENT, Shift, current, failure, number, text};
use crate::ingest::now_ms;
use crate::pipe::{Job, feed};
use crate::race::race;

struct Analysis {
    alert: Value,
    args: Vec<String>,
    brief: Brief,
    claim: Value,
    started: Instant,
}

struct Delivery {
    attempt: u64,
    claim: Value,
    extra: Vec<String>,
    notice: String,
    started: Instant,
}

fn analysis_jobs(shift: &Shift<'_>, alerts: &[Value]) -> Vec<Analysis> {
    alerts
        .iter()
        .filter_map(|alert| {
            if replay(shift, alert) {
                return None;
            }
            let now = now_ms();
            let brief = brief(shift, alert, now);
            let args = if brief.resumed() {
                shift.triage.resume_args(text(alert, "thread_id"))
            } else {
                shift.triage.fresh_args()
            };
            let claim = claim_of(shift, text(alert, "key"), PENDING);
            if shift.ledger.claim_alert(&claim) == 0 {
                return None;
            }
            Some(Analysis {
                alert: alert.clone(),
                args,
                brief,
                claim,
                started: Instant::now(),
            })
        })
        .collect()
}

fn analyze(shift: &Shift<'_>, job: &Analysis, done: &io::Result<Output>) {
    let key = text(&job.claim, "key");
    let topic = text(&job.alert, "topic_id");
    external(
        "analyze",
        job.brief.prompt().len(),
        &outcome(
            done,
            job.started,
            number(&job.alert, "attempts").saturating_add(1),
        ),
    );
    let reply = done
        .as_ref()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| shift.desk.reply(&String::from_utf8_lossy(&output.stdout)));
    if let Some(found) = reply {
        let recorded = shift.ledger.record_analysis(&json!({
            "analysis": found.text(),
            "at_ms": now_ms(),
            "key": key,
            "thread_id": found.thread_id(),
            "topic_id": topic,
            "until_key": job.brief.until_key(),
            "until_ms": job.brief.until_ms(),
        }));
        let advanced = (recorded > 0).then(|| {
            shift.ledger.advance_alert(&json!({
                "analysis": null,
                "from": PENDING,
                "key": key,
                "state": ANALYZED,
            }))
        });
        tracing::info!(
            recorded,
            advanced,
            len = found.text().len(),
            "alert analyzed"
        );
    } else {
        let reason = failure(done);
        let failed = shift.ledger.fail_alert(&failed_of(&job.claim, &reason));
        let dropped = shift.ledger.forget_thread(&json!({"topic_id": topic}));
        tracing::warn!(failed, dropped, reason, "alert analysis failed");
    }
}

fn brief(shift: &Shift<'_>, alert: &Value, now: u64) -> Brief {
    let reusable = !text(alert, "thread_id").is_empty()
        && number(alert, "thread_at_ms") >= now.saturating_sub(shift.triage.session_max_ms());
    let rows = shift
        .ledger
        .topic_lines(&json!({"topic_id": text(alert, "topic_id")}));
    let (rules, _rev) = current(shift);
    shift
        .desk
        .brief(&json!({
            "resume": reusable.then(|| json!({
                "until": number(alert, "until_ms"),
                "until_key": text(alert, "until_key"),
            })),
            "rows": rows,
            "rules": rules,
        }))
        .unwrap_or_default()
}

fn claim_of(shift: &Shift<'_>, key: &str, state: &str) -> Value {
    let now = now_ms();
    json!({
        "at_ms": now,
        "cooldown_since_ms": now.saturating_sub(shift.triage.handoff_cooldown_ms()),
        "hour_since_ms": now.saturating_sub(HOUR_MS),
        "key": key,
        "kind": if state == PENDING { "analysis" } else { "notify" },
        "max_attempts": shift.triage.max_attempts(),
        "max_per_hour": shift.triage.max_handoffs_per_hour(),
        "state": state,
    })
}

fn deliver(shift: &Shift<'_>, job: &Delivery, done: &io::Result<Output>) {
    let key = text(&job.claim, "key");
    external(
        "notify",
        job.notice.len(),
        &outcome(done, job.started, job.attempt),
    );
    if done.as_ref().is_ok_and(|output| output.status.success()) {
        let rows = shift.ledger.advance_alert(&json!({
            "analysis": null,
            "from": ANALYZED,
            "key": key,
            "state": SENT,
        }));
        tracing::info!(rows, "alert sent");
    } else {
        let reason = failure(done);
        let rows = shift.ledger.fail_alert(&failed_of(&job.claim, &reason));
        tracing::warn!(rows, reason, "alert step failed");
    }
}

fn delivery_jobs(shift: &Shift<'_>, alerts: &[Value]) -> Vec<Delivery> {
    alerts
        .iter()
        .filter_map(|alert| {
            let key = text(alert, "key");
            let claim = claim_of(shift, key, ANALYZED);
            if shift.ledger.claim_alert(&claim) == 0 {
                return None;
            }
            Some(Delivery {
                attempt: number(alert, "attempts").saturating_add(1),
                claim,
                extra: [
                    shift.triage.notify_target().value().unwrap_or_default(),
                    format!("--idempotency-key={key}"),
                ]
                .into_iter()
                .collect(),
                notice: shift.desk.notice(alert),
                started: Instant::now(),
            })
        })
        .collect()
}

fn failed_of(claim: &Value, reason: &str) -> Value {
    json!({
        "at_ms": now_ms(),
        "key": claim.get("key"),
        "max_attempts": claim.get("max_attempts"),
        "reason": reason,
        "state": claim.get("state"),
    })
}

fn outcome(done: &io::Result<Output>, started: Instant, attempt: u64) -> Outcome {
    let elapsed = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
    let status = if done.as_ref().is_ok_and(|output| output.status.success()) {
        "success".to_owned()
    } else {
        failure(done)
    };
    let code = done
        .as_ref()
        .ok()
        .and_then(|output| output.status.code())
        .and_then(|code| u16::try_from(code).ok());
    let response = done.as_ref().map_or(0, |output| output.stdout.len());
    Outcome::new(response, status, elapsed)
        .code(code)
        .attempt(attempt)
}

fn release(shift: &Shift<'_>, claim: &Value) {
    let released = shift.ledger.release_alert(claim);
    tracing::debug!(released, "alert claim released");
}

fn replay(shift: &Shift<'_>, alert: &Value) -> bool {
    if text(alert, "analysis").is_empty() {
        return false;
    }
    let advanced = shift.ledger.advance_alert(&json!({
        "analysis": null,
        "from": PENDING,
        "key": text(alert, "key"),
        "state": ANALYZED,
    }));
    tracing::info!(advanced, replayed = true, "alert analyzed");
    true
}

pub(super) async fn run(
    shift: &mut Shift<'_>,
    pending: &[Value],
    analyzed: &[Value],
) -> Option<()> {
    let analyses = analysis_jobs(shift, pending);
    let deliveries = delivery_jobs(shift, analyzed);
    let split = analyses.len();
    let triage = shift.triage;
    let work = join_all(
        analyses
            .iter()
            .map(|job| feed(Job::new(triage.codex(), &job.args, job.brief.prompt())))
            .chain(
                deliveries
                    .iter()
                    .map(|job| feed(Job::new(triage.notify(), &job.extra, &job.notice))),
            ),
    );
    let Some(mut outcomes) = race(&mut *shift.stop, pin!(work)).await else {
        for job in &analyses {
            external(
                "analyze",
                job.brief.prompt().len(),
                &stopped(
                    job.started,
                    number(&job.alert, "attempts").saturating_add(1),
                ),
            );
            release(shift, &job.claim);
        }
        for job in &deliveries {
            external(
                "notify",
                job.notice.len(),
                &stopped(job.started, job.attempt),
            );
            release(shift, &job.claim);
        }
        return None;
    };
    let sent = outcomes.split_off(split);
    for (job, done) in analyses.iter().zip(outcomes) {
        analyze(shift, job, &done);
    }
    for (job, done) in deliveries.iter().zip(sent) {
        deliver(shift, job, &done);
    }
    Some(())
}

fn stopped(started: Instant, attempt: u64) -> Outcome {
    let elapsed = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
    Outcome::new(0, "stopped".to_owned(), elapsed).attempt(attempt)
}
