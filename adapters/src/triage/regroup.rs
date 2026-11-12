use serde_json::json;

use super::{Shift, number, text};
use crate::ingest::now_ms;

const SPLIT: &str = "split";

pub(super) fn regroup(shift: &Shift<'_>) {
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
            "reach": topics.reach(),
            "since_ms": now.saturating_sub(shift.triage.lookback_ms()),
            "threshold": shift.triage.questions().threshold(),
            "to": target,
        }));
        if settled > 0 {
            tracing::info!(serial, kind, settled, "topic change settled");
        }
    }
}
