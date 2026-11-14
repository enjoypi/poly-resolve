use core::pin::pin;

use serde_json::{Value, json};

use super::{SUCCESS, Shift, text};
use crate::ingest::now_ms;
use crate::pipe::{Job, feed};
use crate::race::race;
use crate::trace::finished;

async fn drop_thread(shift: &mut Shift<'_>, doomed: &Value) -> Option<()> {
    let thread = text(doomed, "thread_id").to_owned();
    let extra = vec![thread];
    let work = feed(Job::new(shift.triage.purge(), &extra, ""));
    let done = race(&mut *shift.stop, pin!(work)).await?;
    let status = finished(&done);
    let forgotten = shift.ledger.forget_purged(&json!({
        "at_ms": now_ms(),
        "thread_id": text(doomed, "thread_id"),
        "topic_id": text(doomed, "topic_id"),
    }));
    if status == SUCCESS {
        tracing::info!(forgotten, "codex session purged");
    } else {
        tracing::warn!(forgotten, status, "codex session purge failed");
    }
    Some(())
}

pub(super) async fn purge(shift: &mut Shift<'_>) -> Option<()> {
    let retain = shift.triage.topics().retain();
    let args = json!({
        "at_ms": now_ms(),
        "before_ms": now_ms().saturating_sub(retain.after_ms()),
        "limit": shift.triage.intake(),
    });
    let purged = shift.ledger.purge(&args);
    if purged > 0 {
        tracing::info!(purged, "expired messages purged");
    }
    for doomed in shift.ledger.doomed_threads(&args) {
        drop_thread(shift, &doomed).await?;
    }
    Some(())
}
