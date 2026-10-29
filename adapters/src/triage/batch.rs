use serde_json::{Value, from_str, json};

use super::{HOUR_MS, NONE, Shift, current, failure, number, retry_before, text};
use crate::ingest::now_ms;
use crate::pipe::Job;

const READY: &str = "ready";
const SENT: &str = "sent";
const URGENT: &str = "urgent";

fn alerts(shift: &Shift<'_>, topics: &[Value], batch_id: u64) -> Vec<Value> {
    let now = now_ms();
    topics
        .iter()
        .filter(|topic| text(topic, "urgent") == URGENT)
        .map(|topic| {
            json!({
                "at_ms": now,
                "cooldown_since_ms": now.saturating_sub(shift.triage.handoff_cooldown_ms()),
                "hour_since_ms": now.saturating_sub(HOUR_MS),
                "key": format!("{}@{batch_id}", text(topic, "topic")),
                "max_per_hour": shift.triage.max_handoffs_per_hour(),
                "topic_id": text(topic, "topic"),
            })
        })
        .collect()
}

pub(super) fn seal(shift: &Shift<'_>) {
    let Some(batch_id) = shift
        .ledger
        .sealable(&json!({"after": NONE}))
        .first()
        .map(|row| number(row, "batch_id"))
    else {
        return;
    };
    let lines = shift.ledger.batch_lines(&json!({"batch_id": batch_id}));
    let now = now_ms();
    if lines.is_empty() {
        let rows = shift.ledger.seal(&json!({
            "at_ms": now,
            "id": batch_id,
            "layout": "",
            "payload": "",
            "state": SENT,
            "updated_ms": now,
        }));
        tracing::info!(rows, "batch without approved lines closed");
        return;
    }
    let (rules, _rev) = current(shift);
    let since = now.saturating_sub(shift.triage.session_max_ms());
    let topics = shift.ledger.open_topics(&json!({
        "limit": shift.triage.topics_max_open(),
        "since_ms": since,
    }));
    let attach = shift
        .ledger
        .attach(&json!({"batch_id": batch_id, "since_ms": since}));
    let sealed = shift
        .desk
        .pack(&json!({
            "attach": attach,
            "batch_id": batch_id,
            "lines": lines,
            "rules": rules,
            "topics": topics,
        }))
        .map(|packed| {
            shift.ledger.seal(&json!({
                "at_ms": now,
                "id": batch_id,
                "layout": packed.layout().to_string(),
                "payload": packed.body(),
                "state": READY,
                "updated_ms": NONE,
            }))
        });
    tracing::info!(sealed, lines = lines.len(), "batch sealed");
}

pub(super) async fn send(shift: &mut Shift<'_>) -> Option<()> {
    let now = now_ms();
    let used = shift.ledger.requests_since(&json!({
        "since_ms": now.saturating_sub(HOUR_MS),
    }));
    if used >= u64::from(shift.triage.max_decisions_per_hour()) {
        return Some(());
    }
    let due = shift
        .ledger
        .next_batch(&json!({"retry_before_ms": retry_before(shift)}));
    for batch in &due {
        let triage = shift.triage;
        let done = shift
            .exec(Job::new(triage.decide(), &[], text(batch, "payload")))
            .await?;
        let reply = done
            .as_ref()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned());
        let layout: Value = from_str(text(batch, "layout")).unwrap_or_default();
        let sorted = reply
            .as_deref()
            .and_then(|body| shift.desk.sort(&layout, body));
        let batch_id = number(batch, "id");
        if let Some(found) = sorted {
            let rows = shift.ledger.apply(
                &json!({
                    "at_ms": now,
                    "batch_id": batch_id,
                    "placements": found.placements(),
                    "topics": found.topics(),
                }),
                &alerts(shift, found.topics(), batch_id),
            );
            tracing::info!(rows, topics = found.topics().len(), "batch decided");
        } else {
            let reason = reply.map_or_else(|| failure(&done), |_| "bad_answer".to_owned());
            let rows = shift.ledger.fail_batch(&json!({
                "at_ms": now,
                "id": batch_id,
                "max_attempts": triage.max_attempts(),
                "reason": reason,
            }));
            tracing::warn!(rows, reason, "decision failed");
        }
    }
    Some(())
}
