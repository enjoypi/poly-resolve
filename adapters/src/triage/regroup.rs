use serde_json::json;

use super::{Shift, number, text};
use crate::ingest::now_ms;

const ONE: u32 = 1;
const SPLIT: &str = "split";

fn propose(shift: &Shift<'_>) {
    let topics = shift.triage.topics();
    let candidates = shift.ledger.drift(&json!({
        "merge_after": topics.merge_after(),
        "split_after": topics.split_after(),
    }));
    let changes = if candidates.is_empty() {
        Vec::new()
    } else {
        shift.desk.regroup(&candidates)
    };
    if !changes.is_empty() {
        let proposed = shift
            .ledger
            .propose(&json!({"at_ms": now_ms(), "changes": changes}));
        tracing::info!(proposed, "topic changes proposed");
    }
}

pub(super) fn regroup(shift: &Shift<'_>) {
    if !shift.ledger.unindexed(&json!({"limit": ONE})).is_empty() {
        tracing::debug!("topic changes wait for clues");
        return;
    }
    propose(shift);
    settle(shift);
    suspect(shift);
}

fn settle(shift: &Shift<'_>) {
    let now = now_ms();
    for change in &shift.ledger.changes() {
        let serial = number(change, "id");
        let kind = text(change, "kind");
        let target = if kind == SPLIT {
            format!("s{serial}")
        } else {
            text(change, "into").to_owned()
        };
        let settled = shift.ledger.settle(&json!({
            "at_ms": now,
            "from": text(change, "from"),
            "id": serial,
            "keys": change.get("keys"),
            "kind": kind,
            "reach": shift.triage.topics().reach(),
            "since_ms": now.saturating_sub(shift.triage.lookback_ms()),
            "threshold": shift.triage.questions().threshold(),
            "to": target,
        }));
        if settled > 0 {
            tracing::info!(serial, kind, settled, "topic change settled");
        }
    }
}

fn suspect(shift: &Shift<'_>) {
    let suspected = shift.ledger.suspect(&json!({
        "max_conversations": shift.triage.topics().clues().max_conversations(),
    }));
    if suspected > 0 {
        tracing::info!(suspected, "topics suspected related");
    }
}
