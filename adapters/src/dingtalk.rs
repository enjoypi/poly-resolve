mod bridge;
pub(crate) mod config;
mod dws;

use core::pin::pin;
use core::slice::from_mut;
use core::time::Duration;
use std::process::Output;

use futures_util::stream::{StreamExt as _, iter};
use rusqlite::{Error, ErrorCode};
use serde_json::{Value, from_str};
use tokio::time::sleep;

use self::bridge::{Bridge, Page, Window};
use self::config::Settings;

use self::dws::{Dws, Found, Listener, children, plaintext};
use crate::config::Settings as Shared;
use crate::ingest::{Site, now_ms};
use crate::race::{Stop, race};
use crate::store::{Archive, Scope};

pub(crate) const SOURCE: &str = "dingtalk";

#[derive(Clone, Copy, Debug)]
enum Halt {
    Busy,
    Exit(u8),
}

#[derive(Clone, Copy, Debug)]
enum Retry {
    Failed,
    Progress,
}

struct Run<'run> {
    bridge: Bridge<'run>,
    config: &'run Settings,
    dws: Dws<'run>,
    shared: &'run Shared,
    stop: Stop<'run>,
    store: Archive,
}

impl Run<'_> {
    async fn absorb(&mut self, body: &str) -> Result<(), Halt> {
        let Ok(parsed) = self.bridge.parse_page(body, now_ms()) else {
            tracing::warn!(len = body.len(), "page rejected");
            return Ok(());
        };
        let mut page = Page::parse(&parsed);
        self.assemble(page.envelopes_mut()).await?;
        self.unseal(page.envelopes_mut()).await?;
        let inserted = self
            .store
            .save_page(&page.render())
            .map_err(|err| self.fault(&err))?;
        tracing::info!(
            inserted,
            skipped = page.skipped(),
            incomplete = page.next_cursor().is_some(),
            "reconciled"
        );
        Ok(())
    }

    async fn assemble(&mut self, envelopes: &mut [Value]) -> Result<(), Halt> {
        let ids: Vec<String> = envelopes.iter().filter_map(lookup_of).collect();
        if ids.is_empty() {
            return Ok(());
        }
        let found = self.looked(&ids).await?;
        for envelope in envelopes {
            self.attach(envelope, &found);
        }
        Ok(())
    }

    async fn assembled(&mut self, envelope: String) -> Result<String, Halt> {
        let mut value: Value = from_str(&envelope).unwrap_or_default();
        if lookup_of(&value).is_none() {
            return Ok(envelope);
        }
        self.assemble(from_mut(&mut value)).await?;
        Ok(value.to_string())
    }

    fn attach(&self, envelope: &mut Value, found: &Found) {
        let assembled = lookup_of(envelope)
            .and_then(|ident| found.get(&ident))
            .filter(|kids| !kids.is_empty())
            .and_then(|kids| self.bridge.assemble(&envelope.to_string(), kids));
        if let Some(next) = assembled {
            *envelope = next;
        }
    }

    fn checked(&self, output: &Output) -> Result<Option<String>, Halt> {
        let code = output.status.code();
        if self.dws.manual(code) {
            tracing::error!(code, "dws needs manual action");
            return Err(Halt::Exit(self.shared.manual_exit_code()));
        }
        if !output.status.success() {
            tracing::warn!(code, "dws pull failed");
            return Ok(None);
        }
        Ok(Some(String::from_utf8_lossy(&output.stdout).into_owned()))
    }

    async fn decrypted(&mut self, envelopes: Vec<String>) -> Result<Vec<String>, Halt> {
        let (bridge, dws) = (&self.bridge, &self.dws);
        let mut work = pin!(
            iter(envelopes)
                .map(|envelope| reveal(bridge, dws, envelope))
                .buffered(self.config.dws().parallel())
                .collect::<Vec<String>>()
        );
        race(&mut *self.stop, work.as_mut())
            .await
            .ok_or(Halt::Exit(0))
    }

    async fn drive(&mut self, listener: &mut Listener) -> Result<u64, Halt> {
        self.reconcile().await?;
        self.heal().await?;
        self.pump(listener).await
    }

    fn fault(&self, err: &Error) -> Halt {
        if matches!(
            err.sqlite_error_code(),
            Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked)
        ) {
            tracing::warn!(error = %err, "store busy");
            Halt::Busy
        } else {
            tracing::error!(error = %err, "store failed");
            Halt::Exit(self.shared.failure_exit_code())
        }
    }

    async fn heal(&mut self) -> Result<(), Halt> {
        let sealed = self
            .store
            .sealed(
                Scope::new(SOURCE, self.config.account()),
                self.config.heal_limit(),
            )
            .map_err(|err| self.fault(&err))?;
        let opened = self.decrypted(sealed.clone()).await?;
        for (stale, fresh) in sealed.iter().zip(opened) {
            if fresh != *stale {
                let healed = self
                    .store
                    .replace(stale, &fresh)
                    .map_err(|err| self.fault(&err))?;
                tracing::debug!(healed, "stored message decrypted");
            }
        }
        Ok(())
    }

    async fn ingest(&mut self, line: &str) -> Result<(), Halt> {
        match self.bridge.parse_event(line, now_ms()) {
            Ok(parsed) => {
                let sealed = self.unsealed(parsed).await?;
                let envelope = self.assembled(sealed).await?;
                self.store
                    .save_event(&envelope)
                    .map(|inserted| tracing::debug!(inserted, "event stored"))
                    .map_err(|err| self.fault(&err))
            }
            Err(rejected) => {
                tracing::warn!(len = line.len(), error = ?rejected, "event rejected");
                Ok(())
            }
        }
    }

    async fn looked(&mut self, ids: &[String]) -> Result<Found, Halt> {
        let mut found = Found::new();
        for chunk in ids.chunks(self.config.lookup_batch()) {
            let dws = &self.dws;
            let mut work = pin!(dws.lookup(chunk));
            let Some(done) = race(&mut *self.stop, work.as_mut()).await else {
                return Err(Halt::Exit(0));
            };
            found.extend(done.ok().map_or_default(|output| children(&output)));
        }
        tracing::debug!(
            requested = ids.len(),
            found = found.len(),
            "forwards looked up"
        );
        Ok(found)
    }

    async fn pause(&mut self, millis: u64) -> Option<()> {
        let mut nap = pin!(sleep(Duration::from_millis(millis)));
        race(&mut *self.stop, nap.as_mut()).await
    }

    async fn pull(&mut self, window: &Window) -> Result<Option<String>, Halt> {
        let dws = &self.dws;
        let mut work = pin!(dws.pull(window.start(), window.end()));
        let Some(pulled) = race(&mut *self.stop, work.as_mut()).await else {
            return Err(Halt::Exit(0));
        };
        match pulled {
            Ok(output) => self.checked(&output),
            Err(err) => {
                tracing::warn!(error = %err, "dws pull not started");
                Ok(None)
            }
        }
    }

    async fn pump(&mut self, listener: &mut Listener) -> Result<u64, Halt> {
        let mut received: u64 = 0;
        loop {
            let mut next = pin!(listener.next_line());
            let Some(read) = race(&mut *self.stop, next.as_mut()).await else {
                return Err(Halt::Exit(0));
            };
            match read {
                Ok(Some(line)) => {
                    self.ingest(&line).await?;
                    received = received.saturating_add(1);
                }
                Ok(None) | Err(_) => return Ok(received),
            }
        }
    }

    async fn reconcile(&mut self) -> Result<(), Halt> {
        let Some(window) = self.window()? else {
            return Ok(());
        };
        let Some(body) = self.pull(&window).await? else {
            return Ok(());
        };
        self.absorb(&body).await
    }

    async fn serve(&mut self) -> u8 {
        let initial = self.shared.backoff_initial_ms();
        let mut delay = initial;
        loop {
            let retry = match self.session().await {
                Ok(retry) => retry,
                Err(Halt::Busy) => Retry::Failed,
                Err(Halt::Exit(code)) => return code,
            };
            let wait = match retry {
                Retry::Failed => delay,
                Retry::Progress => initial,
            };
            delay = wait.saturating_mul(2).min(self.shared.backoff_max_ms());
            if self.pause(wait).await.is_none() {
                return 0;
            }
        }
    }

    async fn session(&mut self) -> Result<Retry, Halt> {
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
            return Err(Halt::Exit(self.shared.manual_exit_code()));
        }
        tracing::warn!(received, code, "dws listen ended");
        Ok(if received > 0 {
            Retry::Progress
        } else {
            Retry::Failed
        })
    }

    async fn unseal(&mut self, envelopes: &mut [Value]) -> Result<(), Halt> {
        let opened = self
            .decrypted(envelopes.iter().map(Value::to_string).collect())
            .await?;
        for (envelope, clear) in envelopes.iter_mut().zip(opened) {
            *envelope = from_str(&clear).unwrap_or_default();
        }
        Ok(())
    }

    async fn unsealed(&mut self, envelope: String) -> Result<String, Halt> {
        let opened = self.decrypted(vec![envelope]).await?;
        Ok(opened.into_iter().next().unwrap_or_default())
    }

    fn window(&self) -> Result<Option<Window>, Halt> {
        let to_ms = now_ms();
        let mark = self
            .store
            .watermark(SOURCE, self.config.account())
            .map_err(|err| self.fault(&err))?;
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

fn lookup_of(envelope: &Value) -> Option<String> {
    envelope
        .get("lookup")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

async fn reveal(bridge: &Bridge<'_>, dws: &Dws<'_>, envelope: String) -> String {
    let Some(cipher) = bridge.cipher(&envelope) else {
        return envelope;
    };
    let done = dws.decrypt(&cipher).await;
    if let Some(clear) = done.ok().and_then(|output| plaintext(&output)) {
        tracing::debug!(len = cipher.len(), "message decrypted");
        bridge.reveal(&envelope, &clear).unwrap_or(envelope)
    } else {
        tracing::warn!(len = cipher.len(), "message decrypt failed");
        envelope
    }
}

pub(crate) async fn run(site: Site<'_>, config: &Settings, stop: Stop<'_>) -> u8 {
    let shared = site.settings();
    match Archive::open(shared.store_path(), shared.busy_timeout_ms()) {
        Ok(store) => {
            Run {
                bridge: Bridge::new(config, site.context()),
                config,
                dws: Dws::new(config.dws()),
                shared,
                stop,
                store,
            }
            .serve()
            .await
        }
        Err(err) => {
            tracing::error!(error = %err, "store open failed");
            shared.failure_exit_code()
        }
    }
}
