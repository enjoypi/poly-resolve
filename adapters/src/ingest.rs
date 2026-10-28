use core::fmt::Display;
use core::pin::pin;
use core::time::Duration;
use std::process::Output;
use std::time::{SystemTime, UNIX_EPOCH};

use domain::features::Context;
use tokio::time::sleep;

use crate::bridge::{Bridge, Summary, Window};
use crate::config::Settings;
use crate::dws::{Dws, Listener};
use crate::race::{Stop, race};
use crate::store::Archive;

#[derive(Clone, Copy, Debug)]
enum Retry {
    Failed,
    Progress,
}

struct Run<'run> {
    bridge: Bridge<'run>,
    config: &'run Settings,
    dws: Dws<'run>,
    stop: Stop<'run>,
    store: Archive,
}

impl Run<'_> {
    fn absorb(&self, body: &str) -> Result<(), u8> {
        let Ok(page) = self.bridge.parse_page(body, now_ms()) else {
            tracing::warn!(len = body.len(), "page rejected");
            return Ok(());
        };
        let inserted = self
            .store
            .save_page(&page)
            .map_err(|err| self.fatal(&err))?;
        let summary = Summary::parse(&page);
        tracing::info!(
            inserted,
            skipped = summary.skipped(),
            incomplete = summary.next_cursor().is_some(),
            "reconciled"
        );
        Ok(())
    }

    fn checked(&self, output: &Output) -> Result<Option<String>, u8> {
        let code = output.status.code();
        if self.dws.manual(code) {
            tracing::error!(code, "dws needs manual action");
            return Err(self.config.manual_exit_code());
        }
        if !output.status.success() {
            tracing::warn!(code, "dws pull failed");
            return Ok(None);
        }
        Ok(Some(String::from_utf8_lossy(&output.stdout).into_owned()))
    }

    async fn drive(&mut self, listener: &mut Listener) -> Result<u64, u8> {
        self.reconcile().await?;
        self.pump(listener).await
    }

    fn fatal(&self, err: &dyn Display) -> u8 {
        tracing::error!(error = %err, "store failed");
        self.config.failure_exit_code()
    }

    fn ingest(&self, line: &str) -> Result<(), u8> {
        match self.bridge.parse_event(line, now_ms()) {
            Ok(envelope) => self
                .store
                .save_event(&envelope)
                .map(|inserted| tracing::debug!(inserted, "event stored"))
                .map_err(|err| self.fatal(&err)),
            Err(rejected) => {
                tracing::warn!(len = line.len(), error = ?rejected, "event rejected");
                Ok(())
            }
        }
    }

    async fn pause(&mut self, millis: u64) -> Option<()> {
        let mut nap = pin!(sleep(Duration::from_millis(millis)));
        race(&mut *self.stop, nap.as_mut()).await
    }

    async fn pull(&mut self, window: &Window) -> Result<Option<String>, u8> {
        let dws = &self.dws;
        let mut work = pin!(dws.pull(window.start(), window.end()));
        let Some(pulled) = race(&mut *self.stop, work.as_mut()).await else {
            return Err(0);
        };
        match pulled {
            Ok(output) => self.checked(&output),
            Err(err) => {
                tracing::warn!(error = %err, "dws pull not started");
                Ok(None)
            }
        }
    }

    async fn pump(&mut self, listener: &mut Listener) -> Result<u64, u8> {
        let mut received: u64 = 0;
        loop {
            let mut next = pin!(listener.next_line());
            let Some(read) = race(&mut *self.stop, next.as_mut()).await else {
                return Err(0);
            };
            match read {
                Ok(Some(line)) => {
                    self.ingest(&line)?;
                    received = received.saturating_add(1);
                }
                Ok(None) | Err(_) => return Ok(received),
            }
        }
    }

    async fn reconcile(&mut self) -> Result<(), u8> {
        let Some(window) = self.window()? else {
            return Ok(());
        };
        let Some(body) = self.pull(&window).await? else {
            return Ok(());
        };
        self.absorb(&body)
    }

    async fn serve(&mut self) -> u8 {
        let initial = self.config.backoff_initial_ms();
        let mut delay = initial;
        loop {
            let retry = match self.session().await {
                Ok(retry) => retry,
                Err(code) => return code,
            };
            let wait = match retry {
                Retry::Failed => delay,
                Retry::Progress => initial,
            };
            delay = wait.saturating_mul(2).min(self.config.backoff_max_ms());
            if self.pause(wait).await.is_none() {
                return 0;
            }
        }
    }

    async fn session(&mut self) -> Result<Retry, u8> {
        let mut listener = match self.dws.listen() {
            Ok(listener) => listener,
            Err(err) => {
                tracing::error!(error = %err, "dws listen not started");
                return Ok(Retry::Failed);
            }
        };
        let outcome = self.drive(&mut listener).await;
        let limit = Duration::from_millis(self.config.child_exit_timeout_ms());
        let code = listener.stop(limit).await;
        let received = outcome?;
        if self.dws.manual(code) {
            tracing::error!(code, "dws needs manual action");
            return Err(self.config.manual_exit_code());
        }
        tracing::warn!(received, code, "dws listen ended");
        Ok(if received > 0 {
            Retry::Progress
        } else {
            Retry::Failed
        })
    }

    fn window(&self) -> Result<Option<Window>, u8> {
        let to_ms = now_ms();
        let mark = self
            .store
            .watermark(self.config.source(), self.config.account())
            .map_err(|err| self.fatal(&err))?;
        let from_ms = mark.map_or_else(
            || to_ms.saturating_sub(self.config.initial_backfill_ms()),
            |millis| millis.saturating_sub(self.config.reconcile_overlap_ms()),
        );
        Ok(self
            .bridge
            .window(from_ms, to_ms)
            .map_err(|err| tracing::warn!(error = ?err, "window rejected"))
            .ok())
    }
}

fn now_ms() -> u64 {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
}

pub async fn run(config: &Settings, context: &Context<'_>, stop: Stop<'_>) -> u8 {
    match Archive::open(config.store_path()) {
        Ok(store) => {
            Run {
                bridge: Bridge::new(config, context),
                config,
                dws: Dws::new(config.dws()),
                stop,
                store,
            }
            .serve()
            .await
        }
        Err(err) => {
            tracing::error!(error = %err, "store open failed");
            config.failure_exit_code()
        }
    }
}
