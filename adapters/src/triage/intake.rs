use serde_json::{Value, json};

use super::{Shift, current, number, text};
use crate::ingest::now_ms;

const APPROVED: &str = "approved";
const AUTO: &str = "auto";
const PENDING: &str = "pending";
const QUEUED: &str = "queued";
const SCREENING: &str = "screening";
const SKIPPED: &str = "skipped";

struct Stamp<'stamp> {
    at_ms: u64,
    rev: u64,
    rules: &'stamp Value,
}

pub(super) fn form(shift: &Shift<'_>) {
    let auto = shift.triage.review_auto();
    let screening = shift.triage.screen().is_some();
    let (state, reviewer) = match (screening, auto) {
        (true, _) => (SCREENING, ""),
        (false, true) => (APPROVED, AUTO),
        (false, false) => (PENDING, ""),
    };
    let formed = shift.ledger.form(&json!({
        "at_ms": now_ms(),
        "limit": shift.triage.batch(),
        "reviewer": reviewer,
        "state": state,
    }));
    if formed > 0 {
        tracing::info!(formed, auto, screening, "batch formed");
    }
}

pub(super) fn intake(shift: &Shift<'_>) {
    let now = now_ms();
    let messages = shift.ledger.unlined(&json!({
        "limit": shift.triage.intake(),
        "since_ms": now.saturating_sub(shift.triage.lookback_ms()),
    }));
    let (rules, rev) = current(shift);
    let stamp = Stamp {
        at_ms: now,
        rev,
        rules: &rules,
    };
    for message in &messages {
        open(shift, message, &stamp);
    }
}

fn open(shift: &Shift<'_>, message: &Value, stamp: &Stamp<'_>) {
    let Some(line) = shift.desk.line(message, stamp.rules) else {
        tracing::warn!("line unavailable");
        return;
    };
    let skip = line.skip();
    let opened = shift.ledger.open_line(&json!({
        "at_ms": stamp.at_ms,
        "conversation_id": text(message, "conversation_id"),
        "key": text(message, "key"),
        "occurred_at_ms": number(message, "occurred_at_ms"),
        "reason": skip.unwrap_or_default(),
        "rules_rev": stamp.rev,
        "sender": line.sender(),
        "state": if skip.is_some() { SKIPPED } else { QUEUED },
        "text": line.text(),
    }));
    tracing::debug!(opened, skipped = skip.is_some(), "line opened");
}

pub(super) fn rerender(shift: &Shift<'_>) {
    let (rules, rev) = current(shift);
    let stale = shift.ledger.stale(&json!({
        "limit": shift.triage.intake(),
        "rev": rev,
    }));
    let at_ms = now_ms();
    for line in &stale {
        let restated = shift.ledger.restate(&json!({
            "at_ms": at_ms,
            "key": text(line, "key"),
            "rev": rev,
            "text": shift.desk.masked(&rules, text(line, "text")),
        }));
        tracing::debug!(restated, "line masked again");
    }
}
