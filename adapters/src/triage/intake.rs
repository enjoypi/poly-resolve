use serde_json::{Value, json};

use super::{Shift, current, number, text};
use crate::ingest::now_ms;

const APPROVED: &str = "approved";
const AUTO: &str = "auto";
const SCREENING: &str = "screening";

struct Stamp<'stamp> {
    at_ms: u64,
    rev: u64,
    rules: &'stamp Value,
}

pub(super) fn form(shift: &Shift<'_>) {
    let screening = shift.triage.screen().is_some();
    let (state, reviewer) = if screening {
        (SCREENING, "")
    } else {
        (APPROVED, AUTO)
    };
    let now = now_ms();
    let formed = shift.ledger.form(&json!({
        "at_ms": now,
        "limit": shift.triage.batch(),
        "reviewer": reviewer,
        "state": state,
    }));
    if formed > 0 {
        tracing::info!(formed, screening, "batch formed");
    }
}

pub(super) fn index(shift: &Shift<'_>) {
    let pending = shift
        .ledger
        .unindexed(&json!({"limit": shift.triage.intake()}));
    for message in &pending {
        let Some(clues) = shift.desk.clues(message) else {
            tracing::warn!("clues unavailable");
            shift.fault();
            return;
        };
        let indexed = shift.ledger.index(&json!({
            "clues": clues,
            "key": text(message, "key"),
            "occurred_at_ms": number(message, "occurred_at_ms"),
        }));
        tracing::debug!(indexed, "line indexed");
    }
}

pub(super) fn intake(shift: &Shift<'_>) {
    let now = now_ms();
    let since = shift.ledger.intake_since(&json!({
        "enabled": true,
        "since_ms": now.saturating_sub(shift.triage.lookback_ms()),
    }));
    let messages = since.map_or_default(|since_ms| {
        shift.ledger.unlined(&json!({
            "limit": shift.triage.intake(),
            "since_ms": since_ms,
        }))
    });
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
        shift.fault();
        return;
    };
    let opened = shift.ledger.open_line(&json!({
        "at_ms": stamp.at_ms,
        "conversation_id": text(message, "conversation_id"),
        "key": text(message, "key"),
        "occurred_at_ms": number(message, "occurred_at_ms"),
        "reason": line.skip(),
        "role": line.role(),
        "rules_rev": stamp.rev,
        "sender": line.sender(),
        "state": line.state(),
        "text": line.text(),
    }));
    tracing::debug!(opened, state = line.state(), "line opened");
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
