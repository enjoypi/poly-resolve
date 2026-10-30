use std::io;
use std::process::Output;

use serde_json::{Value, json};

use super::desk::Brief;
use super::{ANALYZED, PENDING, SENT, Shift, current, failure, number, text};
use crate::ingest::now_ms;
use crate::pipe::Job;

pub(super) async fn analyze(shift: &mut Shift<'_>, alert: &Value) -> Option<()> {
    let triage = shift.triage;
    let key = text(alert, "key");
    let topic = text(alert, "topic_id");
    let now = now_ms();
    let brief = brief(shift, alert, now);
    let args = if brief.resumed() {
        triage.resume_args(text(alert, "thread_id"))
    } else {
        triage.fresh_args()
    };
    let claim = claim_of(shift, key, PENDING);
    let Some(done) = attempt(
        shift,
        &claim,
        Job::new(triage.codex(), &args, brief.prompt()),
    )
    .await?
    else {
        return Some(());
    };
    let reply = done
        .as_ref()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| shift.desk.reply(&String::from_utf8_lossy(&output.stdout)));
    if let Some(found) = reply {
        let advanced = shift.ledger.advance_alert(&json!({
            "analysis": found.text(),
            "from": PENDING,
            "key": key,
            "state": ANALYZED,
        }));
        let bound = shift.ledger.bind_thread(&json!({
            "at_ms": now,
            "thread_id": found.thread_id(),
            "topic_id": topic,
            "until_ms": brief.until_ms(),
        }));
        tracing::info!(advanced, bound, len = found.text().len(), "alert analyzed");
    } else {
        let reason = failure(&done);
        let failed = shift.ledger.fail_alert(&failed_of(&claim, &reason));
        let dropped = shift.ledger.forget_thread(&json!({"topic_id": topic}));
        tracing::warn!(failed, dropped, reason, "alert analysis failed");
    }
    Some(())
}

async fn attempt(
    shift: &mut Shift<'_>,
    claim: &Value,
    job: Job<'_>,
) -> Option<Option<io::Result<Output>>> {
    if shift.ledger.claim_alert(claim) == 0 {
        return Some(None);
    }
    let Some(done) = shift.exec(job).await else {
        let released = shift.ledger.release_alert(claim);
        tracing::debug!(released, "alert claim released");
        return None;
    };
    Some(Some(done))
}

fn brief(shift: &Shift<'_>, alert: &Value, now: u64) -> Brief {
    let reusable = !text(alert, "thread_id").is_empty()
        && number(alert, "thread_at_ms") >= now.saturating_sub(shift.triage.session_max_ms());
    let rows = shift
        .ledger
        .topic_lines(&json!({"topic_id": text(alert, "topic_id")}));
    let (rules, _rev) = current(shift);
    shift
        .desk
        .brief(&json!({
            "resume": reusable.then(|| json!({"until": number(alert, "until_ms")})),
            "rows": rows,
            "rules": rules,
        }))
        .unwrap_or_default()
}

fn claim_of(shift: &Shift<'_>, key: &str, state: &str) -> Value {
    json!({
        "at_ms": now_ms(),
        "key": key,
        "max_attempts": shift.triage.max_attempts(),
        "state": state,
    })
}

pub(super) async fn deliver(shift: &mut Shift<'_>, alert: &Value) -> Option<()> {
    let triage = shift.triage;
    let key = text(alert, "key");
    let notice = shift.desk.notice(alert);
    let extra = [
        triage.notify_target().value().unwrap_or_default(),
        format!("--idempotency-key={key}"),
    ];
    let claim = claim_of(shift, key, ANALYZED);
    let Some(done) = attempt(shift, &claim, Job::new(triage.notify(), &extra, &notice)).await?
    else {
        return Some(());
    };
    if done.as_ref().is_ok_and(|output| output.status.success()) {
        let rows = shift.ledger.advance_alert(&json!({
            "analysis": null,
            "from": ANALYZED,
            "key": key,
            "state": SENT,
        }));
        tracing::info!(rows, "alert sent");
    } else {
        let reason = failure(&done);
        let rows = shift.ledger.fail_alert(&failed_of(&claim, &reason));
        tracing::warn!(rows, reason, "alert step failed");
    }
    Some(())
}

fn failed_of(claim: &Value, reason: &str) -> Value {
    json!({
        "at_ms": now_ms(),
        "key": claim.get("key"),
        "max_attempts": claim.get("max_attempts"),
        "reason": reason,
        "state": claim.get("state"),
    })
}
