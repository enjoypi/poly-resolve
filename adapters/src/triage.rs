mod alert;
mod batch;
pub mod config;
mod desk;
mod intake;
mod screen;

use core::pin::pin;
use core::time::Duration;
use std::io;
use std::process::Output;

use domain::features::Context;
use serde_json::{Value, json};
use tokio::time::sleep;

use self::alert::{analyze, deliver};
use self::batch::{seal, send};
use self::config::Settings;
use self::desk::Desk;
use self::intake::{form, intake, rerender};
use self::screen::screen;
use crate::config::Settings as Ingest;
use crate::ingest::now_ms;
use crate::ledger::Ledger;
use crate::pipe::{Job, feed};
use crate::race::{Stop, race};

const ANALYZED: &str = "analyzed";
const HOUR_MS: u64 = 3_600_000;
const MAX_BATCH: u32 = 200;
const MAX_CHOICES: u32 = 254;
pub(super) const NONE: u64 = 0;
const PENDING: &str = "pending";
const SENT: &str = "sent";

#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub enum Lane {
    Alerts,
    Batches,
}

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
    lane: Lane,
    ledger: Ledger,
    stop: Stop<'shift>,
    triage: &'shift Settings,
}

impl Shift<'_> {
    async fn exec(&mut self, job: Job<'_>) -> Option<io::Result<Output>> {
        let mut work = pin!(feed(job));
        race(&mut *self.stop, work.as_mut()).await
    }

    async fn pause(&mut self) -> Option<()> {
        let mut nap = pin!(sleep(Duration::from_millis(self.triage.poll_ms())));
        race(&mut *self.stop, nap.as_mut()).await
    }

    async fn prepare(&mut self) -> Option<()> {
        intake(self);
        rerender(self);
        form(self);
        screen(self).await?;
        seal(self);
        Some(())
    }

    async fn relay(&mut self) -> Option<()> {
        for alert in self.waiting(PENDING) {
            analyze(self, &alert).await?;
        }
        for alert in self.waiting(ANALYZED) {
            deliver(self, &alert).await?;
        }
        Some(())
    }

    async fn serve(&mut self) {
        loop {
            if self.tick().await.is_none() || self.pause().await.is_none() {
                return;
            }
        }
    }

    async fn tick(&mut self) -> Option<()> {
        match self.lane {
            Lane::Alerts => self.relay().await,
            Lane::Batches => {
                self.prepare().await?;
                send(self).await
            }
        }
    }

    fn waiting(&self, state: &str) -> Vec<Value> {
        self.ledger.next_alert(&json!({
            "max_attempts": self.triage.max_attempts(),
            "retry_before_ms": retry_before(self),
            "state": state,
        }))
    }
}

fn current(shift: &Shift<'_>) -> (Value, u64) {
    let learned = shift.ledger.rules(&json!({"after": NONE}));
    let rev = learned
        .iter()
        .map(|rule| number(rule, "id"))
        .max()
        .unwrap_or_default();
    (shift.desk.rules(&learned), rev)
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

fn retry_before(shift: &Shift<'_>) -> u64 {
    now_ms().saturating_sub(shift.triage.retry_after_ms())
}

pub async fn run(setup: Setup<'_>, lane: Lane, stop: Stop<'_>) {
    if !setup.triage.enabled() {
        return;
    }
    match Ledger::open(setup.ingest.store_path(), setup.ingest.busy_timeout_ms()) {
        Ok(ledger) => {
            Shift {
                desk: Desk::new(setup.context, setup.ingest, setup.triage),
                lane,
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

pub fn validate(setup: Setup<'_>) -> Result<(), String> {
    let triage = setup.triage;
    if triage.batch() > MAX_BATCH
        || triage.topics_max_open().saturating_add(triage.batch()) > MAX_CHOICES
    {
        return Err("triage.limits.batch or triage.sizes.topics_max_open too large".to_owned());
    }
    let desk = Desk::new(setup.context, setup.ingest, triage);
    desk.redact(&desk.rules(&[]), "")
        .map(drop)
        .map_err(|_err| "triage.redact.patterns: invalid pattern".to_owned())
}
