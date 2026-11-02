pub(crate) mod config;
mod wire;

use core::convert::Infallible;
use core::pin::pin;
use core::sync::atomic::{AtomicU64, Ordering};
use core::time::Duration;
use std::sync::{Mutex, PoisonError};

use domain::features::{Call, Context, Origin, Target as Route, dispatch};
use http::Uri;
use serde_json::{Value, from_str, json};
use tokio::time::{Instant, sleep, timeout, timeout_at};

use self::config::Settings;
use self::wire::{Conn, Reply, quoted};
use crate::config::Settings as Shared;
use crate::ingest::cycle::{Backoff, Halt, Retry, fault};
use crate::ingest::{Site, now_ms};
use crate::net::{Dial, Layer, Link, open};
use crate::race::{Stop, race};
use crate::store::Archive;
use crate::trace::{Caller, Outcome, external};

const AUTH: &str = "imap:auth";
static CONNECTIONS: AtomicU64 = AtomicU64::new(0);
const DAY_MS: u64 = 86_400_000;
pub(crate) const SOURCE: &str = "imap";
const TRANSPORT: &str = "adapters";

#[derive(Clone, Copy)]
struct Ask<'ask> {
    action: &'ask str,
    request: &'ask Value,
    session: &'ask str,
}

enum Cut {
    Halt(Halt),
    Net(String),
}

#[derive(Clone, Copy)]
struct Mail<'mail> {
    internal: &'mail str,
    raw: &'mail [u8],
    session: &'mail str,
    uid: u64,
}

struct Mailbox {
    capabilities: String,
    conn: Conn,
    seen: u64,
    validity: u64,
}

struct Run<'run> {
    stop: Stop<'run>,
    worker: Worker<'run>,
}

impl Run<'_> {
    async fn pause(&mut self, millis: u64) -> Option<()> {
        let mut nap = pin!(sleep(Duration::from_millis(millis)));
        race(&mut *self.stop, nap.as_mut()).await
    }

    async fn serve(&mut self) -> u8 {
        let shared = self.worker.shared;
        let mut backoff = Backoff::new(shared);
        loop {
            let outcome = {
                let mut work = pin!(self.worker.session());
                race(&mut *self.stop, work.as_mut()).await
            };
            let retry = match outcome {
                None => return 0,
                Some(Ok(retry)) => retry,
                Some(Err(Halt::Busy)) => Retry::Failed,
                Some(Err(Halt::Exit(code))) => return code,
            };
            if self.pause(backoff.next(shared, retry)).await.is_none() {
                return 0;
            }
        }
    }
}

#[derive(Default)]
struct Tally {
    received: u64,
    rounds: u64,
}

struct Worker<'run> {
    config: &'run Settings,
    context: &'run Context<'run>,
    shared: &'run Shared,
    store: Mutex<Archive>,
}

impl Worker<'_> {
    fn call(&self, ask: Ask<'_>) -> Option<Value> {
        let body = ask.request.to_string();
        let call = Call::new(
            Origin::new(TRANSPORT, Some(ask.session), None),
            Route::new("ingest", ask.action),
            &body,
        );
        let reply = dispatch(self.context, &call).ok()?;
        from_str(&reply).ok()
    }

    async fn fetch(&self, mailbox: &mut Mailbox, set: &str) -> Result<u64, Cut> {
        let bytes = self.config.limits().message_bytes();
        let command = format!("UID FETCH {set} (UID INTERNALDATE BODY.PEEK[]<0.{bytes}>)");
        let reply = mailbox
            .conn
            .ask(&command, self.limit())
            .await
            .map_err(Cut::Net)?;
        let mut saved: u64 = 0;
        for found in reply.untagged() {
            let uid = number_after(found.text(), "UID ");
            let Some(raw) = found.literals().first().filter(|_| uid > mailbox.seen) else {
                continue;
            };
            let internal = found
                .text()
                .split_once("INTERNALDATE \"")
                .and_then(|(_, rest)| rest.split_once('"'))
                .map_or("", |(stamp, _)| stamp);
            let mail = Mail {
                internal,
                raw,
                session: mailbox.conn.session(),
                uid,
            };
            saved = saved.saturating_add(self.keep(mail, mailbox.validity)?);
            mailbox.seen = mailbox.seen.max(uid);
        }
        Ok(saved)
    }

    async fn follow(&self, tally: &mut Tally) -> Result<Infallible, Cut> {
        let mut mailbox = self.open().await?;
        let date = self.since()?;
        let found = mailbox
            .conn
            .ask(&format!("UID SEARCH SINCE {date}"), self.limit())
            .await
            .map_err(Cut::Net)?;
        let mut uids: Vec<u64> = found
            .untagged()
            .iter()
            .filter_map(|line| line.text().strip_prefix("* SEARCH"))
            .flat_map(|rest| rest.split_whitespace().filter_map(|uid| uid.parse().ok()))
            .collect();
        uids.sort_unstable();
        let limits = self.config.limits();
        let skip = uids.len().saturating_sub(limits.search_max());
        let base = mailbox.seen;
        mailbox.seen = 0;
        for chunk in uids
            .get(skip..)
            .unwrap_or_default()
            .chunks(limits.fetch_batch())
        {
            let set: Vec<String> = chunk.iter().map(ToString::to_string).collect();
            let saved = self.fetch(&mut mailbox, &set.join(",")).await?;
            tally.received = tally.received.saturating_add(saved);
        }
        mailbox.seen = mailbox.seen.max(base);
        loop {
            self.wait(&mut mailbox).await?;
            tally.rounds = tally.rounds.saturating_add(1);
            let set = format!("{}:*", mailbox.seen.saturating_add(1));
            let saved = self.fetch(&mut mailbox, &set).await?;
            tally.received = tally.received.saturating_add(saved);
        }
    }

    async fn idle(&self, conn: &mut Conn) -> Result<(), String> {
        let mark = conn.mark("IDLE", "");
        let ended = self.idling(conn).await;
        conn.note(mark, ended.clone().unwrap_or_else(|reason| reason));
        ended.map(drop)
    }

    async fn idling(&self, conn: &mut Conn) -> Result<String, String> {
        let limit = self.limit();
        let tag = conn.start("IDLE").await;
        let ready = timeout(limit, conn.response())
            .await
            .map_err(|_elapsed| "timeout:idle".to_owned())??;
        if !ready.text().starts_with('+') {
            return Err("imap:idle".to_owned());
        }
        let span = Duration::from_millis(self.config.idle_ms());
        let deadline = Instant::now()
            .checked_add(span)
            .unwrap_or_else(Instant::now);
        let mut reason = "refresh";
        while let Ok(heard) = timeout_at(deadline, conn.response()).await {
            let text = heard?.text().to_owned();
            if text.starts_with("* BYE") {
                return Err("imap:bye".to_owned());
            }
            if text.ends_with(" EXISTS") {
                reason = "exists";
                break;
            }
        }
        conn.send("DONE").await;
        timeout(limit, conn.finish(&tag))
            .await
            .map_err(|_elapsed| "timeout:idle".to_owned())?
            .and_then(Reply::checked)
            .map(|_reply| reason.to_owned())
    }

    fn keep(&self, mail: Mail<'_>, validity: u64) -> Result<u64, Cut> {
        let request = json!({
            "account": self.config.account(),
            "internal": mail.internal,
            "limits": self.config.limits().mime(),
            "raw": mail.raw,
            "received_at_ms": now_ms(),
            "source": SOURCE,
            "style": self.config.style(),
            "uid": mail.uid,
            "uid_validity": validity,
        });
        let parsed = self.call(Ask {
            action: "parse_mail",
            request: &request,
            session: mail.session,
        });
        let skip = parsed
            .as_ref()
            .and_then(|found| found.get("skip"))
            .and_then(Value::as_str);
        let envelope = parsed
            .as_ref()
            .filter(|_| skip.is_none())
            .and_then(|found| found.get("envelope"));
        let saved = match envelope {
            Some(found) => self
                .store
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .save_event(&found.to_string())
                .map_err(|err| Cut::Halt(fault(self.shared, &err)))?,
            None => 0,
        };
        let status = match (parsed.as_ref(), skip) {
            (None, _) => "rejected".to_owned(),
            (Some(_), Some(reason)) => format!("skip:{reason}"),
            (Some(_), None) => "success".to_owned(),
        };
        let caller = Caller::new(SOURCE, SOURCE, self.config.account()).session(mail.session);
        let outcome = Outcome::new(saved, status, 0)
            .within(caller)
            .params(format!("{validity}:{}", mail.uid));
        external("mail", mail.raw.len(), &outcome);
        Ok(u64::try_from(saved).unwrap_or_default())
    }

    const fn limit(&self) -> Duration {
        Duration::from_millis(self.config.timeout_ms())
    }

    async fn open(&self) -> Result<Mailbox, Cut> {
        let (mut conn, capabilities) = login(self.config).await.map_err(Cut::Net)?;
        let folder = quoted(self.config.mailbox()).map_err(Cut::Net)?;
        let selected = conn
            .ask(&format!("SELECT {folder}"), self.limit())
            .await
            .map_err(Cut::Net)?;
        let code = |name: &str| {
            selected
                .untagged()
                .iter()
                .map(|line| number_after(line.text(), name))
                .max()
                .unwrap_or_default()
        };
        Ok(Mailbox {
            capabilities,
            conn,
            seen: code("[UIDNEXT ").saturating_sub(1),
            validity: code("[UIDVALIDITY "),
        })
    }

    async fn session(&self) -> Result<Retry, Halt> {
        let mut tally = Tally::default();
        let Err(cut) = self.follow(&mut tally).await;
        let account = self.config.account();
        match cut {
            Cut::Halt(halt) => Err(halt),
            Cut::Net(reason) if reason == AUTH => {
                tracing::error!(account, reason, "imap login refused");
                Ok(Retry::Stalled)
            }
            Cut::Net(reason) => {
                let received = tally.received;
                let rounds = tally.rounds;
                tracing::warn!(account, reason, received, rounds, "imap session ended");
                Ok(if received > 0 || rounds > 0 {
                    Retry::Progress
                } else {
                    Retry::Failed
                })
            }
        }
    }

    fn since(&self) -> Result<String, Cut> {
        let mark = self
            .store
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .watermark(SOURCE, self.config.account())
            .map_err(|err| Cut::Halt(fault(self.shared, &err)))?;
        let at_ms = mark
            .map_or_else(
                || now_ms().saturating_sub(self.config.initial_backfill_ms()),
                |found| found.saturating_sub(self.config.reconcile_overlap_ms()),
            )
            .saturating_sub(DAY_MS);
        let request = json!({"at_ms": at_ms});
        Ok(self
            .call(Ask {
                action: "since",
                request: &request,
                session: "",
            })
            .and_then(|reply| reply.get("date").and_then(Value::as_str).map(str::to_owned))
            .unwrap_or_default())
    }

    async fn wait(&self, mailbox: &mut Mailbox) -> Result<(), Cut> {
        if capable(&mailbox.capabilities, "IDLE") {
            self.idle(&mut mailbox.conn).await.map_err(Cut::Net)
        } else {
            sleep(Duration::from_millis(self.config.poll_ms())).await;
            Ok(())
        }
    }
}

fn capable(capabilities: &str, name: &str) -> bool {
    capabilities
        .split_whitespace()
        .any(|found| found.eq_ignore_ascii_case(name))
}

pub(crate) async fn check(config: &Settings) -> Result<(), String> {
    let (mut conn, _capabilities) = login(config)
        .await
        .map_err(|reason| format!("imap:{}: {reason}", config.account()))?;
    let limit = Duration::from_millis(config.timeout_ms());
    drop(conn.ask("LOGOUT", limit).await);
    Ok(())
}

async fn connect(config: &Settings, caller: Caller<'_>) -> Result<Link, String> {
    let target = config.target()?;
    let dial = Dial::new(target.host(), target.port(), target.layer()).with_proxy(target.proxy());
    let limit = Duration::from_millis(config.timeout_ms());
    let started = Instant::now();
    let mut stage = "connect";
    let link = timeout(limit, open(&dial, &mut stage))
        .await
        .map_err(|_elapsed| format!("timeout:{stage}"))
        .and_then(|opened| opened);
    let layer = if target.layer() == Layer::Tls {
        "tls"
    } else {
        "plain"
    };
    let route = target.proxy().and_then(Uri::scheme_str).unwrap_or("direct");
    let elapsed = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
    let status = link
        .as_ref()
        .map_or_else(Clone::clone, |_| "success".to_owned());
    let outcome = Outcome::new(0, status, elapsed)
        .within(caller)
        .params(format!("{layer}+{route}"));
    external("connect", 0, &outcome);
    link
}

async fn login(config: &Settings) -> Result<(Conn, String), String> {
    let session = format!(
        "{}#{}",
        config.account(),
        CONNECTIONS.fetch_add(1, Ordering::Relaxed)
    );
    let caller = Caller::new(SOURCE, SOURCE, config.account()).session(&session);
    let link = connect(config, caller).await?;
    let limits = config.limits();
    let limit = Duration::from_millis(config.timeout_ms());
    let bytes = usize::try_from(limits.message_bytes()).unwrap_or(usize::MAX);
    let mut conn =
        Conn::new(link, limits.line_bytes(), bytes).named(config.account(), session.clone());
    let greeting = timeout(limit, conn.response())
        .await
        .map_err(|_elapsed| "timeout:greeting".to_owned())??;
    if !greeting.text().starts_with("* OK") {
        return Err("imap:greeting".to_owned());
    }
    let listed = conn.ask("CAPABILITY", limit).await?;
    let capabilities = listed
        .untagged()
        .iter()
        .find(|line| line.text().starts_with("* CAPABILITY"))
        .map_or_else(String::new, |line| line.text().to_owned());
    if capable(&capabilities, "ID") {
        drop(conn.ask("ID (\"name\" \"poly-resolve\")", limit).await?);
    }
    let user = quoted(&config.user())?;
    let password = quoted(&config.password())?;
    drop(conn.ask(&format!("LOGIN {user} {password}"), limit).await?);
    Ok((conn, capabilities))
}

fn number_after(text: &str, marker: &str) -> u64 {
    text.split_once(marker)
        .and_then(|(_, rest)| {
            rest.split(|letter: char| !letter.is_ascii_digit())
                .next()
                .and_then(|digits| digits.parse().ok())
        })
        .unwrap_or_default()
}

pub(crate) async fn run(site: Site<'_>, config: &Settings, stop: Stop<'_>) -> u8 {
    let shared = site.settings();
    match Archive::open(shared.store_path(), shared.busy_timeout_ms()) {
        Ok(store) => {
            Run {
                stop,
                worker: Worker {
                    config,
                    context: site.context(),
                    shared,
                    store: Mutex::new(store),
                },
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
