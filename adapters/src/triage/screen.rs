use std::time::Instant;

use serde_json::{Value, json, to_string};

use super::config::Screen;
use super::desk::Screened;
use super::trace::{Outcome, external};
use super::{Shift, retry_before, text};
use crate::http::Call;
use crate::ingest::now_ms;

const APPROVED: &str = "approved";
const AUTO: &str = "auto";
const PENDING: &str = "pending";
const REJECTED: &str = "rejected";
const SCREEN: &str = "screen";

struct Prepared {
    body: String,
    candidates: Vec<Value>,
    topics: Vec<Value>,
}

fn apply(shift: &Shift<'_>, verdicts: &[Screened], now: u64) {
    let auto = shift.triage.review_auto();
    for verdict in verdicts {
        let flagged = !verdict.flag().is_empty();
        let (state, reviewer, reason) = match (auto, flagged) {
            (true, true) => (REJECTED, "", SCREEN),
            (true, false) => (APPROVED, AUTO, ""),
            (false, _) => (PENDING, "", ""),
        };
        let rows = shift.ledger.screened(&json!({
            "at_ms": now,
            "flag": verdict.flag(),
            "hints": to_string(verdict.hints()).unwrap_or_default(),
            "key": verdict.key(),
            "reason": reason,
            "reviewer": reviewer,
            "state": state,
            "text": verdict.text(),
        }));
        tracing::debug!(rows, flagged, "line screened");
    }
}

fn request(shift: &Shift<'_>, setup: &Screen, lines: &[Value]) -> Prepared {
    let triage = shift.triage;
    let topics = shift.ledger.open_topics(&json!({
        "limit": triage.topics_max_open(),
        "since_ms": now_ms().saturating_sub(triage.session_max_ms()),
    }));
    let candidates: Vec<Value> = lines
        .iter()
        .map(|line| json!({"key": text(line, "key"), "text": text(line, "text")}))
        .collect();
    let body = shift.desk.screen(&json!({
        "instructions": setup.instructions(),
        "lines": candidates,
        "model": setup.model(),
        "topics": topics,
    }));
    Prepared {
        body,
        candidates,
        topics,
    }
}

pub(super) async fn screen(shift: &mut Shift<'_>) -> Option<()> {
    let triage = shift.triage;
    let Some(setup) = triage.screen() else {
        return Some(());
    };
    let lines = shift.ledger.screening(&json!({
        "limit": triage.batch(),
        "retry_before_ms": retry_before(shift),
    }));
    if lines.is_empty() {
        return Some(());
    }
    let prepared = request(shift, setup, &lines);
    let url = match setup.http().local_url() {
        Ok(url) => url,
        Err(_err) => {
            let keys: Vec<&str> = lines.iter().map(|line| text(line, "key")).collect();
            let deferred = shift
                .ledger
                .defer(&json!({"at_ms": now_ms(), "keys": keys}));
            tracing::warn!(deferred, "screening failed");
            return Some(());
        }
    };
    let key = setup.http().key();
    let call = Call::new(&url, &prepared.body, setup.http().timeout_ms()).with_key(key.as_deref());
    let started = Instant::now();
    let Some(done) = shift.post(&call).await else {
        let elapsed = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
        external(
            "screen",
            prepared.body.len(),
            &Outcome::new(0, "stopped".to_owned(), elapsed),
        );
        return None;
    };
    let elapsed = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
    let outcome = done.as_ref().map_or_else(
        |reason| Outcome::new(0, reason.clone(), elapsed),
        |reply| {
            Outcome::new(reply.body().len(), "success".to_owned(), elapsed)
                .code(Some(reply.status()))
        },
    );
    external("screen", prepared.body.len(), &outcome);
    let now = now_ms();
    let found = done.ok().and_then(|reply| {
        shift.desk.screened(&json!({
            "lines": prepared.candidates,
            "min_chars": triage.learn_min_chars(),
            "placeholder": setup.placeholder(),
            "response": reply.body(),
            "topics": prepared.topics,
        }))
    });
    if let Some(verdicts) = found {
        apply(shift, &verdicts, now);
    } else {
        let keys: Vec<&str> = lines.iter().map(|line| text(line, "key")).collect();
        let deferred = shift.ledger.defer(&json!({"at_ms": now, "keys": keys}));
        tracing::warn!(deferred, "screening failed");
    }
    Some(())
}
