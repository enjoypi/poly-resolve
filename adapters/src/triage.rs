mod alert;
mod batch;
pub mod config;
mod desk;
pub(crate) mod endpoint;
mod intake;
mod screen;

use alloc::collections::BTreeMap;
use core::cell::RefCell;
use core::pin::pin;
use core::time::Duration;

use domain::features::Context;
use serde_json::{Value, json};
use tokio::time::sleep;

use self::alert::run as relay_alerts;
use self::batch::{seal, send};
use self::config::Settings;
use self::desk::Desk;
use self::endpoint::Http;
use self::intake::{form, intake, rerender};
use self::screen::screen;
use crate::config::Settings as Ingest;
use crate::http::{Call, Client as HttpClient, Reply};
use crate::ingest::now_ms;
use crate::ledger::Ledger;
use crate::race::{Stop, race};
use crate::trace::{Caller, failure};

const ANALYZED: &str = "analyzed";
const HOUR_MS: u64 = 3_600_000;
const MAX_BATCH: u32 = 200;
const MAX_CHOICES: u32 = 254;
pub(super) const NONE: u64 = 0;
const PENDING: &str = "pending";
const SENT: &str = "sent";
pub(super) const TRIAGE: Caller<'static> = Caller::new("external", "triage", "");

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
    http: HttpClient,
    lane: Lane,
    ledger: Ledger,
    notes: RefCell<BTreeMap<&'static str, String>>,
    stop: Stop<'shift>,
    triage: &'shift Settings,
}

impl Shift<'_> {
    fn gate(&self, state: &str) -> Value {
        let now = now_ms();
        json!({
            "cooldown_since_ms": now.saturating_sub(self.triage.handoff_cooldown_ms()),
            "hour_since_ms": now.saturating_sub(HOUR_MS),
            "max_attempts": self.triage.max_attempts(),
            "max_per_hour": self.triage.max_handoffs_per_hour(),
            "parallel": self.triage.parallel(),
            "retry_before_ms": retry_before(self),
            "spread_ms": spread(self),
            "state": state,
        })
    }

    fn noted(&self, topic: &'static str, state: &str) -> bool {
        let previous = self.notes.borrow_mut().insert(topic, state.to_owned());
        !state.is_empty() && previous.as_deref() != Some(state)
    }

    async fn pause(&mut self) -> Option<()> {
        let mut nap = pin!(sleep(Duration::from_millis(self.triage.poll_ms())));
        race(&mut *self.stop, nap.as_mut()).await
    }

    async fn post(&mut self, call: &Call<'_>) -> Option<Result<Reply, String>> {
        let mut work = pin!(self.http.post(call));
        race(&mut *self.stop, work.as_mut()).await
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
        let woken = self.ledger.wake_alerts();
        if woken > 0 {
            tracing::debug!(woken, "alerts activated");
        }
        let held = self.ledger.held(&self.gate(PENDING));
        let first = held.first();
        let reason = first.map_or("", |row| text(row, "reason"));
        if self.noted("handoff", reason) {
            tracing::info!(
                reason,
                alerts = first.map_or(NONE, |row| number(row, "count")),
                "handoff held"
            );
        }
        let pending = self.waiting(PENDING);
        let analyzed = self.waiting(ANALYZED);
        relay_alerts(self, &pending, &analyzed).await
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
        self.ledger.next_alert(&self.gate(state))
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

async fn knock(client: &HttpClient, http: &Http) -> Result<(), String> {
    let url = http.url()?;
    let proxy = http.proxy().ok().flatten();
    let call = Call::new(&url, "", http.timeout_ms()).with_proxy(proxy.as_ref());
    client.probe(&call).await
}

fn number(row: &Value, key: &str) -> u64 {
    row.get(key).and_then(Value::as_u64).unwrap_or_default()
}

pub async fn reach(triage: &Settings) -> Result<(), String> {
    let client = HttpClient::new();
    knock(&client, triage.decide())
        .await
        .map_err(|err| format!("triage.decide: unreachable {err}"))?;
    match triage.codex().endpoint() {
        Some(http) => knock(&client, http)
            .await
            .map_err(|err| format!("triage.codex.endpoint: unreachable {err}")),
        None => Ok(()),
    }
}

fn retry_before(shift: &Shift<'_>) -> u64 {
    now_ms().saturating_sub(shift.triage.retry_after_ms())
}

pub async fn run(setup: Setup<'_>, lane: Lane, stop: Stop<'_>) {
    match Ledger::open(setup.ingest.store_path(), setup.ingest.busy_timeout_ms()) {
        Ok(ledger) => {
            if !setup.triage.enabled() {
                let _: Option<u64> = ledger.intake_since(&json!({
                    "enabled": false,
                    "since_ms": NONE,
                }));
                return;
            }
            Shift {
                desk: Desk::new(setup.context, setup.ingest, setup.triage),
                http: HttpClient::new(),
                lane,
                ledger,
                notes: RefCell::default(),
                stop,
                triage: setup.triage,
            }
            .serve()
            .await;
        }
        Err(err) => tracing::error!(error = %err, "triage ledger open failed"),
    }
}

const fn spread(shift: &Shift<'_>) -> u64 {
    shift.triage.retry_after_ms() >> 2
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
    if triage.backlog_max_ms() < triage.lookback_ms() {
        return Err("triage.limits.backlog_max_ms below lookback_ms".to_owned());
    }
    triage
        .decide()
        .url()
        .map(drop)
        .map_err(|err| format!("triage.decide: {err}"))?;
    if let Some(screen) = triage.screen() {
        screen
            .http()
            .local_url()
            .map(drop)
            .map_err(|err| format!("triage.screen.http: {err}"))?;
    }
    let desk = Desk::new(setup.context, setup.ingest, triage);
    desk.redact(&desk.rules(&[]), "")
        .map(drop)
        .map_err(|_err| "triage.redact.patterns: invalid pattern".to_owned())
}
