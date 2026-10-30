use std::time::Instant;

use serde_json::{Value, from_str, json};

use super::trace::{Outcome, external};
use super::{HOUR_MS, NONE, Shift, current, number, retry_before, text};
use crate::http::Call;
use crate::ingest::now_ms;

const READY: &str = "ready";
const SENT: &str = "sent";
const URGENT: &str = "urgent";

fn alerts(_shift: &Shift<'_>, topics: &[Value], batch_id: u64) -> Vec<Value> {
    let now = now_ms();
    topics
        .iter()
        .filter(|topic| text(topic, "urgent") == URGENT)
        .map(|topic| {
            json!({
                "at_ms": now,
                "key": format!("{}@{batch_id}", text(topic, "topic")),
                "topic_id": text(topic, "topic"),
            })
        })
        .collect()
}

fn judge(shift: &Shift<'_>, batch: &Value, reply: Result<String, String>) {
    let now = now_ms();
    let batch_id = number(batch, "id");
    let layout: Value = from_str(text(batch, "layout")).unwrap_or_default();
    let sorted = reply
        .as_deref()
        .ok()
        .and_then(|body| shift.desk.sort(&layout, body));
    match (sorted, reply) {
        (Some(found), Ok(body)) => {
            let saved = shift
                .ledger
                .answer(&json!({"batch_id": batch_id, "body": body}));
            if saved == 0 {
                tracing::warn!(batch_id, "decision answer store failed");
                return;
            }
            let rows = shift.ledger.apply(
                &json!({
                    "at_ms": now,
                    "batch_id": batch_id,
                    "placements": found.placements(),
                    "topics": found.topics(),
                }),
                &alerts(shift, found.topics(), batch_id),
            );
            tracing::info!(saved, rows, topics = found.topics().len(), "batch decided");
        }
        (_, outcome) => {
            let reason = outcome.map_or_else(|failed| failed, |_| "bad_answer".to_owned());
            let rows = shift.ledger.fail_batch(&json!({
                "at_ms": now,
                "id": batch_id,
                "max_attempts": shift.triage.max_attempts(),
                "reason": reason,
            }));
            tracing::warn!(rows, reason, "decision failed");
        }
    }
}

async fn request(shift: &mut Shift<'_>, batch: &Value) -> Option<Option<Result<String, String>>> {
    let triage = shift.triage;
    let setup = triage.decide();
    let url = match setup.url() {
        Ok(url) => url,
        Err(err) => return Some(Some(Err(err))),
    };
    let key = setup.key();
    let call = Call::new(&url, text(batch, "payload"), setup.timeout_ms()).with_key(key.as_deref());
    let claim = json!({"at_ms": now_ms(), "id": number(batch, "id"), "max_attempts": triage.max_attempts()});
    if shift.ledger.claim_batch(&claim) == 0 {
        return Some(None);
    }
    let started = Instant::now();
    let attempt = number(batch, "attempts").saturating_add(1);
    let Some(done) = shift.post(&call).await else {
        let elapsed = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
        external(
            "decide",
            text(batch, "payload").len(),
            &Outcome::new(0, "stopped".to_owned(), elapsed).attempt(attempt),
        );
        let released = shift.ledger.release_batch(&claim);
        tracing::debug!(released, "batch claim released");
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
    external(
        "decide",
        text(batch, "payload").len(),
        &outcome.attempt(attempt),
    );
    Some(Some(done.map(|reply| reply.body().to_owned())))
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
    let due = shift.ledger.next_batch(&json!({
        "max_attempts": shift.triage.max_attempts(),
        "retry_before_ms": retry_before(shift),
    }));
    for batch in &due {
        let stored = batch.get("answer").and_then(Value::as_str);
        if stored.is_none() {
            let used = shift.ledger.requests_since(&json!({
                "since_ms": now_ms().saturating_sub(HOUR_MS),
            }));
            if used >= u64::from(shift.triage.max_decisions_per_hour()) {
                return Some(());
            }
        }
        let reply = match stored {
            Some(body) => Some(Ok(body.to_owned())),
            None => request(shift, batch).await?,
        };
        if let Some(outcome) = reply {
            judge(shift, batch, outcome);
        }
    }
    Some(())
}
