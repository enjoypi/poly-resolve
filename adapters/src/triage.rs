pub mod config;
mod desk;

use core::pin::pin;
use core::time::Duration;
use std::io;
use std::process::Output;

use domain::features::Context;
use serde_json::{Value, json};
use tokio::time::sleep;

use self::config::Settings;
use self::desk::Desk;
use crate::config::Settings as Ingest;
use crate::ingest::now_ms;
use crate::ledger::Ledger;
use crate::pipe::{Job, feed};
use crate::race::{Stop, race};

const ANALYZED: &str = "analyzed";
const HOUR_MS: u64 = 3_600_000;
const PENDING: &str = "pending";
const SENT: &str = "sent";

#[derive(Clone, Copy, Debug)]
pub struct Setup<'setup> {
    context: &'setup Context<'setup>,
    ingest: &'setup Ingest,
    triage: &'setup Settings,
}

impl<'setup> Setup<'setup> {
    #[must_use = "配置需交给 run"]
    pub const fn new(
        ingest: &'setup Ingest,
        triage: &'setup Settings,
        context: &'setup Context<'setup>,
    ) -> Self {
        Self {
            context,
            ingest,
            triage,
        }
    }
}

struct Shift<'shift> {
    desk: Desk<'shift>,
    ingest: &'shift Ingest,
    ledger: Ledger,
    stop: Stop<'shift>,
    triage: &'shift Settings,
}

impl Shift<'_> {
    async fn analyze(&mut self, handoff: &Value) -> Option<()> {
        let trigger = number(handoff, "trigger_ms");
        let lines = self.ledger.conversation(&json!({
            "account": self.ingest.account(),
            "conversation_id": text(handoff, "conversation_id"),
            "limit": self.triage.context_messages(),
            "source": self.ingest.source(),
            "until_ms": trigger,
        }));
        let prompt = self.desk.brief(&lines, trigger);
        let triage = self.triage;
        let done = self.exec(Job::new(triage.codex(), &[], &prompt)).await?;
        let analysis = done
            .as_ref()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| clip(&output.stdout, triage.result_max_chars()))
            .filter(|reply| !reply.is_empty());
        match analysis {
            Some(reply) => {
                let rows = self.ledger.advance(&json!({
                    "analysis": reply,
                    "key": text(handoff, "key"),
                    "state": ANALYZED,
                }));
                tracing::info!(rows, len = reply.len(), "handoff analyzed");
            }
            None => self.failed(handoff, &done),
        }
        Some(())
    }

    async fn decide(&mut self, message: &Value, now: u64) -> Option<()> {
        let key = text(message, "key");
        let asked = self.desk.ask(message);
        let Some(body) = asked.body() else {
            let rows = self.ledger.settle(&json!({
                "at_ms": now,
                "key": key,
                "probability": null,
                "reason": asked.skip(),
                "state": "skipped",
            }));
            tracing::debug!(rows, reason = asked.skip(), "message skipped");
            return Some(());
        };
        let triage = self.triage;
        let done = self.exec(Job::new(triage.decide(), &[], body)).await?;
        let Some(output) = done.as_ref().ok().filter(|output| output.status.success()) else {
            let reason = failure(&done);
            let rows = self
                .ledger
                .fail(&json!({"at_ms": now, "key": key, "reason": reason}));
            tracing::warn!(rows, reason, "decision failed");
            return Some(());
        };
        self.scored(message, &String::from_utf8_lossy(&output.stdout));
        Some(())
    }

    async fn decide_batch(&mut self) -> Option<()> {
        let now = now_ms();
        let used = self.ledger.decisions_since(&json!({
            "since_ms": now.saturating_sub(HOUR_MS),
        }));
        let room = u64::from(self.triage.max_decisions_per_hour()).saturating_sub(used);
        let messages = self.ledger.pending(&json!({
            "account": self.ingest.account(),
            "limit": room.min(u64::from(self.triage.batch())),
            "max_attempts": self.triage.max_attempts(),
            "retry_before_ms": now.saturating_sub(self.triage.retry_after_ms()),
            "since_ms": now.saturating_sub(self.triage.lookback_ms()),
            "source": self.ingest.source(),
        }));
        for message in &messages {
            self.decide(message, now).await?;
        }
        Some(())
    }

    async fn deliver(&mut self, handoff: &Value) -> Option<()> {
        let triage = self.triage;
        let key = text(handoff, "key");
        let notice = self.desk.notice(handoff);
        let extra = [
            triage.notify_target().value().unwrap_or_default(),
            format!("--idempotency-key={key}"),
        ];
        let done = self
            .exec(Job::new(triage.notify(), &extra, &notice))
            .await?;
        if done.as_ref().is_ok_and(|output| output.status.success()) {
            let rows = self
                .ledger
                .advance(&json!({"analysis": null, "key": key, "state": SENT}));
            tracing::info!(rows, "handoff sent");
        } else {
            self.failed(handoff, &done);
        }
        Some(())
    }

    async fn exec(&mut self, job: Job<'_>) -> Option<io::Result<Output>> {
        let mut work = pin!(feed(job));
        race(&mut *self.stop, work.as_mut()).await
    }

    fn failed(&self, handoff: &Value, done: &io::Result<Output>) {
        let reason = failure(done);
        let rows = self.ledger.fail_handoff(&json!({
            "at_ms": now_ms(),
            "key": text(handoff, "key"),
            "max_attempts": self.triage.max_attempts(),
            "reason": reason,
        }));
        tracing::warn!(rows, reason, "handoff step failed");
    }

    async fn pause(&mut self) -> Option<()> {
        let mut nap = pin!(sleep(Duration::from_millis(self.triage.poll_ms())));
        race(&mut *self.stop, nap.as_mut()).await
    }

    fn scored(&self, message: &Value, reply: &str) {
        let (key, now) = (text(message, "key"), now_ms());
        let Some(score) = self.desk.score(reply) else {
            let rows = self.ledger.settle(&json!({
                "at_ms": now,
                "key": key,
                "probability": null,
                "reason": "bad_answer",
                "state": "invalid",
            }));
            tracing::warn!(rows, "decision answer unusable");
            return;
        };
        let rows = self.ledger.settle(&json!({
            "at_ms": now,
            "key": key,
            "probability": score.probability(),
            "reason": "",
            "state": "scored",
        }));
        tracing::debug!(rows, probability = score.probability(), "message scored");
        if score.urgent() {
            let opened = self.ledger.open_handoff(&json!({
                "at_ms": now,
                "conversation_id": text(message, "conversation_id"),
                "cooldown_since_ms": now.saturating_sub(self.triage.handoff_cooldown_ms()),
                "hour_since_ms": now.saturating_sub(HOUR_MS),
                "key": key,
                "max_per_hour": self.triage.max_handoffs_per_hour(),
            }));
            tracing::info!(opened, probability = score.probability(), "urgent message");
        }
    }

    async fn serve(&mut self) {
        loop {
            if self.tick().await.is_none() || self.pause().await.is_none() {
                return;
            }
        }
    }

    async fn tick(&mut self) -> Option<()> {
        self.decide_batch().await?;
        for handoff in self.waiting(PENDING) {
            self.analyze(&handoff).await?;
        }
        for handoff in self.waiting(ANALYZED) {
            self.deliver(&handoff).await?;
        }
        Some(())
    }

    fn waiting(&self, state: &str) -> Vec<Value> {
        self.ledger.next_handoff(&json!({
            "retry_before_ms": now_ms().saturating_sub(self.triage.retry_after_ms()),
            "state": state,
        }))
    }
}

fn clip(bytes: &[u8], max: u32) -> String {
    String::from_utf8_lossy(bytes)
        .trim()
        .chars()
        .take(usize::try_from(max).unwrap_or(usize::MAX))
        .collect()
}

fn failure(done: &io::Result<Output>) -> String {
    done.as_ref().map_or_else(
        |err| format!("io:{:?}", err.kind()),
        |output| format!("exit:{}", output.status.code().unwrap_or_default()),
    )
}

fn number(row: &Value, key: &str) -> u64 {
    row.get(key).and_then(Value::as_u64).unwrap_or_default()
}

pub async fn run(setup: Setup<'_>, stop: Stop<'_>) {
    if !setup.triage.enabled() {
        return;
    }
    match Ledger::open(setup.ingest.store_path()) {
        Ok(ledger) => {
            Shift {
                desk: Desk::new(setup.context, setup.ingest, setup.triage),
                ingest: setup.ingest,
                ledger,
                stop,
                triage: setup.triage,
            }
            .serve()
            .await;
        }
        Err(err) => tracing::error!(error = %err, "triage ledger open failed"),
    }
}

fn text<'row>(row: &'row Value, key: &str) -> &'row str {
    row.get(key).and_then(Value::as_str).unwrap_or_default()
}
