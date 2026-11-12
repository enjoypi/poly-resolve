use alloc::collections::BTreeSet;
use std::time::Instant;

use serde_json::{Value, from_str, json};

use super::TRIAGE;
use super::desk::{Packed, Sorted};
use super::{NONE, Shift, current, number, pace, retry_in, text};
use crate::http::Call;
use crate::ingest::now_ms;
use crate::trace::{Outcome, external};

const AUTO: &str = "auto";
const READY: &str = "ready";
const SENT: &str = "sent";
const SILENT: &str = r#"{"answers":[]}"#;

fn alerts(shift: &Shift<'_>, found: &Sorted, batch_id: u64) -> Vec<Value> {
    let now = now_ms();
    let lookback = shift.triage.lookback_ms();
    let lines = shift.ledger.batch_lines(&json!({"batch_id": batch_id}));
    let fresh: Vec<&str> = lines
        .iter()
        .filter(|line| {
            now.saturating_sub(number(line, "occurred_at_ms")) <= lookback
                && text(line, "role") != AUTO
        })
        .map(|line| text(line, "key"))
        .collect();
    let live: BTreeSet<&str> = found
        .placements()
        .iter()
        .filter(|placement| fresh.contains(&text(placement, "key")))
        .map(|placement| text(placement, "topic"))
        .filter(|topic| !topic.is_empty())
        .collect();
    live.into_iter()
        .map(|topic| {
            json!({
                "at_ms": now,
                "key": format!("{topic}@{batch_id}"),
                "threshold": shift.triage.questions().threshold(),
                "topic_id": topic,
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
                    "gap_ms": shift.triage.session_gap_ms(),
                    "placements": found.placements(),
                    "reach": shift.triage.topics().reach(),
                    "relations": found.relations(),
                    "topics": found.topics(),
                }),
                &alerts(shift, &found, batch_id),
            );
            tracing::info!(saved, rows, topics = found.topics().len(), "batch decided");
        }
        (_, outcome) => {
            let reason = outcome.map_or_else(|failed| failed, |_| "bad_answer".to_owned());
            let max_attempts = shift.triage.max_attempts();
            let rows = shift.ledger.fail_batch(&json!({
                "at_ms": now,
                "id": batch_id,
                "max_attempts": max_attempts,
                "reason": reason,
            }));
            let attempt = number(batch, "attempts").saturating_add(1);
            let limit =
                u64::from(max_attempts).saturating_mul(number(batch, "revivals").saturating_add(1));
            tracing::warn!(
                batch_id,
                rows,
                reason,
                attempt,
                revivals = number(batch, "revivals"),
                exhausted = attempt >= limit,
                retry_in_ms = retry_in(shift, attempt),
                "decision failed"
            );
        }
    }
}

fn packed(shift: &Shift<'_>, batch_id: u64, rules: &Value) -> Option<Packed> {
    let lines = shift.ledger.batch_lines(&json!({"batch_id": batch_id}));
    let topics = shift
        .ledger
        .open_topics(&json!({"limit": shift.triage.topics().max_open()}));
    let attach = shift.ledger.attach(&json!({"batch_id": batch_id}));
    let linked = shift.ledger.linked(&json!({"batch_id": batch_id}));
    shift.desk.pack(&json!({
        "attach": attach,
        "batch_id": batch_id,
        "lines": lines,
        "linked": linked,
        "rules": rules,
        "topics": topics,
    }))
}

fn repacked(shift: &Shift<'_>, batch: &Value) -> Option<Value> {
    let batch_id = number(batch, "id");
    let (rules, rev) = current(shift);
    let Some(found) = packed(shift, batch_id, &rules) else {
        tracing::warn!(batch_id, "batch repack failed");
        shift.fault();
        return None;
    };
    Some(json!({
        "attempts": number(batch, "attempts"),
        "id": batch_id,
        "revivals": number(batch, "revivals"),
        "layout": found.layout().to_string(),
        "payload": found.body(),
        "questions": found.questions(),
        "rev": rev,
    }))
}

async fn request(shift: &mut Shift<'_>, batch: &Value) -> Option<Option<Result<String, String>>> {
    let triage = shift.triage;
    let setup = triage.decide();
    let url = match setup.url() {
        Ok(url) => url,
        Err(err) => return Some(Some(Err(err))),
    };
    let key = setup.key();
    let proxy = setup.proxy().ok().flatten();
    let call = Call::new(&url, text(batch, "payload"), setup.timeout_ms())
        .with_key(key.as_deref())
        .with_proxy(proxy.as_ref());
    let claim = json!({
        "at_ms": now_ms(),
        "id": number(batch, "id"),
        "layout": text(batch, "layout"),
        "max_attempts": triage.max_attempts(),
        "payload": text(batch, "payload"),
        "rev": number(batch, "rev"),
    });
    if shift.ledger.claim_batch(&claim) == 0 {
        tracing::info!(batch_id = number(batch, "id"), "batch claim skipped");
        return Some(None);
    }
    let started = Instant::now();
    let attempt = number(batch, "attempts").saturating_add(1);
    let Some(done) = shift.post(&call).await else {
        let elapsed = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
        external(
            "decide",
            text(batch, "payload").len(),
            &Outcome::new(0, "stopped".to_owned(), elapsed)
                .within(TRIAGE)
                .attempt(attempt),
        );
        let released = shift.ledger.release_batch(&claim);
        tracing::debug!(released, "batch claim released");
        return None;
    };
    let elapsed = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
    let outcome = done.as_ref().map_or_else(
        |reason| Outcome::new(0, reason.clone(), elapsed).within(TRIAGE),
        |reply| {
            Outcome::new(reply.body().len(), "success".to_owned(), elapsed)
                .within(TRIAGE)
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

fn revive(shift: &Shift<'_>) {
    let revived = shift.ledger.revive(&json!({
        "max_revivals": shift.triage.max_revivals(),
        "pace": pace(shift),
    }));
    if revived > 0 {
        tracing::info!(revived, "failed batch revived");
    }
}

pub(super) fn seal(shift: &Shift<'_>) {
    let Some(batch_id) = shift
        .ledger
        .sealable(&json!({"after": NONE}))
        .first()
        .map(|row| number(row, "batch_id"))
    else {
        let stuck = shift.ledger.stuck(&json!({"after": NONE}));
        let first = stuck.first();
        let state = first
            .map_or_default(|row| format!("{}@{}", text(row, "reason"), number(row, "batch_id")));
        if shift.noted("seal", &state) {
            tracing::info!(state, "batch waiting");
        }
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
    let sealed = packed(shift, batch_id, &rules).map(|packed| {
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
    revive(shift);
    let due = shift.ledger.next_batch(&json!({
        "max_attempts": shift.triage.max_attempts(),
        "pace": pace(shift),
    }));
    for batch in &due {
        if let Some(body) = batch.get("answer").and_then(Value::as_str) {
            judge(shift, batch, Ok(body.to_owned()));
            continue;
        }
        let Some(fresh) = repacked(shift, batch) else {
            continue;
        };
        if number(&fresh, "questions") == NONE {
            judge(shift, &fresh, Ok(SILENT.to_owned()));
            continue;
        }
        if let Some(outcome) = request(shift, &fresh).await? {
            judge(shift, &fresh, outcome);
        }
    }
    Some(())
}
