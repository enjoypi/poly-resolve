use alloc::sync::Arc;
use core::pin::pin;
use std::sync::OnceLock;
use std::time::Instant;

use futures_util::stream::{StreamExt as _, iter};
use http::Uri;
use serde_json::{Value, from_value, json, to_string};

use super::TRIAGE;
use super::config::Screen;
use super::desk::Screened;
use super::{FAILED, SUCCESS, Shift, number, pace, retry_in, text};
use crate::http::{Call, Client, Reply};
use crate::ingest::now_ms;
use crate::race::race;
use crate::trace::{Outcome, external};

const APPROVED: &str = "approved";
const AUTO: &str = "auto";
const BAD_ANSWER: &str = "bad_answer";
const MISSING: &str = "missing";
const RISKY: &str = "risky";
const RULES: &str = "rules";
const UNPREPARED: &str = "unprepared";
const UNSCREENED: &str = "unscreened";
const WITHHELD: &str = "withheld";

struct Failure {
    line: Value,
    reason: String,
}

struct Job {
    body: String,
    line: Value,
    started: OnceLock<Instant>,
}

struct Landing {
    done: Result<Reply, String>,
    job: Arc<Job>,
}

struct Round<'round> {
    jobs: Vec<Arc<Job>>,
    key: Option<String>,
    setup: &'round Screen,
    topics: Vec<Value>,
    url: Uri,
}

fn abandon(shift: &Shift<'_>, failure: &Failure) {
    let verdict: Screened = from_value(json!({
        "flag": UNSCREENED,
        "hints": [],
        "key": text(&failure.line, "key"),
        "spans": [],
        "text": text(&failure.line, "text"),
    }))
    .unwrap_or_default();
    tracing::warn!(
        key = text(&failure.line, "key"),
        attempt = number(&failure.line, "attempts").saturating_add(1),
        reason = failure.reason,
        "line screening abandoned"
    );
    apply(shift, &verdict);
}

fn apply(shift: &Shift<'_>, verdict: &Screened) {
    let flagged = !verdict.flag().is_empty();
    let (state, reviewer, reason) = if !flagged {
        (APPROVED, AUTO, "")
    } else if verdict.flag() == UNSCREENED {
        (WITHHELD, "", UNSCREENED)
    } else {
        (WITHHELD, "", RISKY)
    };
    let kind = if flagged {
        verdict.flag()
    } else if verdict.spans().is_empty() {
        ""
    } else {
        RULES
    };
    let rows = shift.ledger.screened(&json!({
        "at_ms": now_ms(),
        "flag": verdict.flag(),
        "hints": to_string(verdict.hints()).unwrap_or_default(),
        "key": verdict.key(),
        "kind": kind,
        "reason": reason,
        "reviewer": reviewer,
        "spans": to_string(verdict.spans()).unwrap_or_default(),
        "state": state,
        "text": verdict.text(),
    }));
    tracing::debug!(rows, flagged, "line screened");
}

fn candidate(line: &Value) -> Value {
    json!({"key": text(line, "key"), "text": text(line, "text")})
}

fn defer(shift: &Shift<'_>, failed: &[Failure]) {
    if failed.is_empty() {
        return;
    }
    let keys: Vec<&str> = failed
        .iter()
        .map(|failure| text(&failure.line, "key"))
        .collect();
    for failure in failed {
        let attempt = number(&failure.line, "attempts").saturating_add(1);
        tracing::warn!(
            key = text(&failure.line, "key"),
            attempt,
            reason = failure.reason,
            retry_in_ms = retry_in(shift, attempt),
            "line screening failed"
        );
    }
    let deferred = shift
        .ledger
        .defer(&json!({"at_ms": now_ms(), "keys": keys}));
    tracing::debug!(deferred, "lines deferred");
}

fn elapsed(job: &Job) -> u64 {
    job.started.get().map_or(0, |started| {
        u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
    })
}

async fn fly(shift: &mut Shift<'_>, round: &Round<'_>) -> Option<()> {
    let http = &shift.http;
    let mut replies = iter(round.jobs.iter().cloned())
        .map(|job| post(http, round, job))
        .buffer_unordered(round.setup.parallel());
    let mut failed: Vec<Failure> = Vec::new();
    let mut landed: Vec<String> = Vec::new();
    loop {
        let Some(next) = race(&mut *shift.stop, pin!(replies.next())).await else {
            grounded(&round.jobs, &landed);
            return None;
        };
        let Some((job, done)) = next else {
            break;
        };
        landed.push(text(&job.line, "key").to_owned());
        let line = job.line.clone();
        match land(shift, round, Landing { done, job }) {
            Ok(()) => {}
            Err(reason) if reason == BAD_ANSWER => abandon(shift, &Failure { line, reason }),
            Err(reason) => failed.push(Failure { line, reason }),
        }
    }
    drop(replies);
    defer(shift, &failed);
    Some(())
}

fn grounded(jobs: &[Arc<Job>], landed: &[String]) {
    for job in jobs.iter().filter(|job| {
        job.started.get().is_some() && !landed.iter().any(|key| key == text(&job.line, "key"))
    }) {
        external(
            "screen",
            job.body.len(),
            &Outcome::new(0, "stopped".to_owned(), elapsed(job)).within(TRIAGE),
        );
    }
}

fn land(shift: &Shift<'_>, round: &Round<'_>, landing: Landing) -> Result<(), String> {
    let Landing { done, job } = landing;
    let attempt = number(&job.line, "attempts").saturating_add(1);
    external(
        "screen",
        job.body.len(),
        &outcome(&done, elapsed(&job)).attempt(attempt),
    );
    let counted = shift.ledger.note_call(&json!({
        "at_ms": now_ms(),
        "attempt": attempt,
        "key": text(&job.line, "key"),
        "kind": "screen",
        "state": if done.is_ok() { SUCCESS } else { FAILED },
    }));
    tracing::debug!(counted, "call settled");
    let reply = done?;
    let verdict = shift
        .desk
        .screened(&json!({
            "lines": [candidate(&job.line)],
            "marks": shift.triage.marks(),
            "min_chars": round.setup.min_chars(),
            "placeholder": round.setup.placeholder(),
            "response": reply.body(),
            "topics": round.topics,
        }))
        .and_then(|found| found.into_iter().next())
        .filter(|verdict| verdict.flag() != MISSING)
        .ok_or_else(|| BAD_ANSWER.to_owned())?;
    apply(shift, &verdict);
    Ok(())
}

fn outcome(done: &Result<Reply, String>, spent: u64) -> Outcome<'static> {
    done.as_ref().map_or_else(
        |reason| Outcome::new(0, reason.clone(), spent).within(TRIAGE),
        |reply| {
            Outcome::new(reply.body().len(), "success".to_owned(), spent)
                .within(TRIAGE)
                .code(Some(reply.status()))
        },
    )
}

async fn post(
    http: &Client,
    round: &Round<'_>,
    job: Arc<Job>,
) -> (Arc<Job>, Result<Reply, String>) {
    let _: &Instant = job.started.get_or_init(Instant::now);
    let timeout = round.setup.http().timeout_ms();
    let call = Call::new(&round.url, &job.body, timeout).with_key(round.key.as_deref());
    let done = http.post(&call).await;
    (job, done)
}

fn prepare<'round>(
    shift: &Shift<'_>,
    setup: &'round Screen,
    lines: &[Value],
) -> Option<Round<'round>> {
    let instructions = shift.desk.prompt("screen")?;
    let url = setup.http().local_url().ok()?;
    let triage = shift.triage;
    let topics = shift
        .ledger
        .open_topics(&json!({"limit": triage.topics().max_open()}));
    let jobs = lines
        .iter()
        .map(|line| {
            Arc::new(Job {
                body: shift.desk.screen(&json!({
                    "instructions": instructions,
                    "lines": [candidate(line)],
                    "max_tokens": setup.max_tokens(),
                    "model": setup.model(),
                    "topics": topics,
                })),
                line: line.clone(),
                started: OnceLock::new(),
            })
        })
        .collect();
    Some(Round {
        jobs,
        key: setup.http().key(),
        setup,
        topics,
        url,
    })
}

pub(super) async fn screen(shift: &mut Shift<'_>) -> Option<()> {
    let triage = shift.triage;
    let Some(setup) = triage.screen() else {
        return Some(());
    };
    let lines = shift.ledger.screening(&json!({
        "limit": triage.batch(),
        "pace": pace(shift),
    }));
    if lines.is_empty() {
        return Some(());
    }
    let Some(round) = prepare(shift, setup, &lines) else {
        let failed: Vec<Failure> = lines
            .iter()
            .map(|line| Failure {
                line: line.clone(),
                reason: UNPREPARED.to_owned(),
            })
            .collect();
        defer(shift, &failed);
        return Some(());
    };
    fly(shift, &round).await
}
