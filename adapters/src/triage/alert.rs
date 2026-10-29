use serde_json::{Value, json};

use super::{ANALYZED, SENT, Shift, current, failure, number, text};
use crate::ingest::now_ms;
use crate::pipe::Job;

pub(super) async fn analyze(shift: &mut Shift<'_>, alert: &Value) -> Option<()> {
    let triage = shift.triage;
    let key = text(alert, "key");
    let topic = text(alert, "topic_id");
    let thread = text(alert, "thread_id");
    let now = now_ms();
    let reusable = !thread.is_empty()
        && number(alert, "thread_at_ms") >= now.saturating_sub(triage.session_max_ms());
    let rows = shift.ledger.topic_lines(&json!({"topic_id": topic}));
    let (rules, _rev) = current(shift);
    let brief = shift
        .desk
        .brief(&json!({
            "resume": reusable.then(|| json!({"until": number(alert, "until_ms")})),
            "rows": rows,
            "rules": rules,
        }))
        .unwrap_or_default();
    let args = if brief.resumed() {
        triage.resume_args(thread)
    } else {
        triage.fresh_args()
    };
    let done = shift
        .exec(Job::new(triage.codex(), &args, brief.prompt()))
        .await?;
    let reply = done
        .as_ref()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| shift.desk.reply(&String::from_utf8_lossy(&output.stdout)));
    if let Some(found) = reply {
        let advanced = shift.ledger.advance_alert(&json!({
            "analysis": found.text(),
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
        let failed = shift.ledger.fail_alert(&json!({
            "at_ms": now,
            "key": key,
            "max_attempts": triage.max_attempts(),
            "reason": reason,
        }));
        let dropped = shift.ledger.forget_thread(&json!({"topic_id": topic}));
        tracing::warn!(failed, dropped, reason, "alert analysis failed");
    }
    Some(())
}

pub(super) async fn deliver(shift: &mut Shift<'_>, alert: &Value) -> Option<()> {
    let triage = shift.triage;
    let key = text(alert, "key");
    let notice = shift.desk.notice(alert);
    let extra = [
        triage.notify_target().value().unwrap_or_default(),
        format!("--idempotency-key={key}"),
    ];
    let done = shift
        .exec(Job::new(triage.notify(), &extra, &notice))
        .await?;
    if done.as_ref().is_ok_and(|output| output.status.success()) {
        let rows = shift
            .ledger
            .advance_alert(&json!({"analysis": null, "key": key, "state": SENT}));
        tracing::info!(rows, "alert sent");
    } else {
        let reason = failure(&done);
        let rows = shift.ledger.fail_alert(&json!({
            "at_ms": now_ms(),
            "key": key,
            "max_attempts": triage.max_attempts(),
            "reason": reason,
        }));
        tracing::warn!(rows, reason, "alert step failed");
    }
    Some(())
}
