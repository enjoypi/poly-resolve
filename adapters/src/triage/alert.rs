use core::pin::pin;
use std::io;
use std::process::Output;
use std::time::Instant;

use futures_util::future::join_all;
use serde_json::{Value, json};

use super::TRIAGE;
use super::desk::{Brief, Reply};
use super::{ANALYZED, PENDING, QUIET, SENT, Shift, current, failure, number, retry_in, text};
use crate::ingest::now_ms;
use crate::pipe::{Job, feed};
use crate::race::race;
use crate::trace::{Outcome, external, finished};

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
            let brief = brief(shift, alert)?;
            let args = if brief.resumed() {
                shift.triage.resume_args(text(alert, "thread_id"))
            } else {
                shift.triage.fresh_args()
            };
            let claim = claim_of(shift, alert, PENDING);
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
        .and_then(|output| {
            shift.desk.reply(
                &String::from_utf8_lossy(&output.stdout),
                text(&job.alert, "state"),
            )
        });
    if let Some(found) = reply {
        conclude(shift, job, &found);
    } else {
        let reason = failure(done);
        let failed = shift.ledger.fail_alert(&failed_of(&job.claim, &reason));
        let dropped = shift.ledger.forget_thread(&json!({"topic_id": topic}));
        let attempt = number(&job.alert, "attempts").saturating_add(1);
        tracing::warn!(
            key,
            failed,
            dropped,
            reason,
            attempt,
            exhausted = attempt >= u64::from(shift.triage.max_attempts()),
            retry_in_ms = retry_in(shift, attempt),
            "alert analysis failed"
        );
    }
}

fn brief(shift: &Shift<'_>, alert: &Value) -> Option<Brief> {
    let reusable = !text(alert, "thread_id").is_empty() && number(alert, "thread_at_ms") > 0;
    let rows = shift
        .ledger
        .topic_lines(&json!({"topic_id": text(alert, "topic_id")}));
    let (rules, _rev) = current(shift);
    shift.desk.brief(&json!({
        "rows": rows,
        "rules": rules,
        "thread": reusable.then(|| text(alert, "thread_id")),
    }))
}

fn claim_of(shift: &Shift<'_>, alert: &Value, state: &str) -> Value {
    let now = now_ms();
    json!({
        "at_ms": now,
        "created_at_ms": alert.get("created_at_ms"),
        "key": alert.get("key"),
        "kind": if state == PENDING { "analysis" } else { "notify" },
        "max_attempts": shift.triage.max_attempts(),
        "state": state,
    })
}

fn conclude(shift: &Shift<'_>, job: &Analysis, found: &Reply) {
    let key = text(&job.claim, "key");
    let topic = text(&job.alert, "topic_id");
    let recorded = shift.ledger.record_analysis(&json!({
        "analysis": found.text(),
        "at_ms": now_ms(),
        "changes": found.changes(),
        "key": key,
        "status": found.status(),
        "thread_id": found.thread_id(),
        "title": found.title(),
        "topic_id": topic,
    }));
    let quiet = found.quiet()
        || shift.ledger.superseded(&json!({
            "created_at_ms": job.alert.get("created_at_ms"),
            "key": key,
            "topic_id": found.thread_id(),
        }));
    let advanced = (recorded > 0).then(|| {
        shift.ledger.advance_alert(&json!({
            "analysis": null,
            "at_ms": now_ms(),
            "from": PENDING,
            "key": key,
            "state": if quiet { QUIET } else { ANALYZED },
        }))
    });
    tracing::info!(
        recorded,
        advanced,
        quiet,
        len = found.text().len(),
        "alert analyzed"
    );
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
            "at_ms": now_ms(),
            "from": ANALYZED,
            "key": key,
            "state": SENT,
        }));
        tracing::info!(rows, "alert sent");
    } else {
        let reason = failure(done);
        let rows = shift.ledger.fail_alert(&failed_of(&job.claim, &reason));
        tracing::warn!(
            key,
            rows,
            reason,
            attempt = job.attempt,
            exhausted = job.attempt >= u64::from(shift.triage.max_attempts()),
            retry_in_ms = retry_in(shift, job.attempt),
            "alert step failed"
        );
    }
}

fn delivery_jobs(shift: &Shift<'_>, alerts: &[Value]) -> Vec<Delivery> {
    alerts
        .iter()
        .filter_map(|alert| {
            let key = text(alert, "key");
            let claim = claim_of(shift, alert, ANALYZED);
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

fn outcome(done: &io::Result<Output>, started: Instant, attempt: u64) -> Outcome<'static> {
    let elapsed = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
    let status = finished(done);
    let code = done
        .as_ref()
        .ok()
        .and_then(|output| output.status.code())
        .and_then(|code| u16::try_from(code).ok());
    let response = done.as_ref().map_or(0, |output| output.stdout.len());
    Outcome::new(response, status, elapsed)
        .within(TRIAGE)
        .code(code)
        .attempt(attempt)
}

pub(super) async fn recap(shift: &mut Shift<'_>) -> Option<()> {
    let merged = shift.ledger.supersede_notices();
    if merged > 0 {
        tracing::info!(merged, "older notices merged");
    }
    let waiting = if shift.ticks == 0 {
        shift.ledger.backlog()
    } else {
        Vec::new()
    };
    let claims = reserved(shift, &waiting);
    if claims.is_empty() {
        return Some(());
    }
    summarize(shift, &waiting, &claims).await
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
        "at_ms": now_ms(),
        "from": PENDING,
        "key": text(alert, "key"),
        "state": ANALYZED,
    }));
    tracing::info!(advanced, replayed = true, "alert analyzed");
    true
}

fn reserved(shift: &Shift<'_>, waiting: &[Value]) -> Vec<Value> {
    if waiting.len() < 2 {
        return Vec::new();
    }
    let claims: Vec<Value> = waiting
        .iter()
        .map(|alert| claim_of(shift, alert, ANALYZED))
        .collect();
    if claims
        .iter()
        .any(|claim| shift.ledger.claim_alert(claim) == 0)
    {
        for claim in &claims {
            release(shift, claim);
        }
        return Vec::new();
    }
    claims
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

fn stopped(started: Instant, attempt: u64) -> Outcome<'static> {
    let elapsed = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
    Outcome::new(0, "stopped".to_owned(), elapsed)
        .within(TRIAGE)
        .attempt(attempt)
}

async fn summarize(shift: &mut Shift<'_>, waiting: &[Value], claims: &[Value]) -> Option<()> {
    let stamp = waiting
        .iter()
        .map(|alert| number(alert, "created_at_ms"))
        .max()
        .unwrap_or_default();
    let extra = vec![
        shift.triage.notify_target().value().unwrap_or_default(),
        format!("--idempotency-key=summary@{stamp}"),
    ];
    let notice = shift.desk.recap(waiting);
    let started = Instant::now();
    let triage = shift.triage;
    let work = join_all([feed(Job::new(triage.notify(), &extra, &notice))]);
    let Some(outcomes) = race(&mut *shift.stop, pin!(work)).await else {
        external("notify", notice.len(), &stopped(started, 1));
        for claim in claims {
            release(shift, claim);
        }
        return None;
    };
    for (done, claim) in outcomes
        .iter()
        .flat_map(|done| claims.iter().map(move |claim| (done, claim)))
    {
        let job = Delivery {
            attempt: 1,
            claim: claim.clone(),
            extra: Vec::new(),
            notice: String::new(),
            started,
        };
        deliver(shift, &job, done);
    }
    tracing::info!(topics = claims.len(), "alerts summarized");
    Some(())
}
