use serde_json::{Value, json, to_string};

use super::config::Screen;
use super::desk::Screened;
use super::{Shift, retry_before, text};
use crate::ingest::now_ms;
use crate::pipe::Job;

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
    let done = shift
        .exec(Job::new(setup.exec(), &[], &prepared.body))
        .await?;
    let now = now_ms();
    let found = done
        .as_ref()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| {
            shift.desk.screened(&json!({
                "lines": prepared.candidates,
                "min_chars": triage.learn_min_chars(),
                "placeholder": setup.placeholder(),
                "response": String::from_utf8_lossy(&output.stdout),
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
