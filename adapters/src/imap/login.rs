use core::sync::atomic::{AtomicU64, Ordering};
use core::time::Duration;

use http::Uri;
use tokio::time::{Instant, timeout};

use super::SOURCE;
use super::config::Settings;
use super::wire::{Conn, quoted};
use crate::net::{Dial, Layer, Link, open};
use crate::trace::{Caller, Outcome, external};

static CONNECTIONS: AtomicU64 = AtomicU64::new(0);

pub(super) fn capable(capabilities: &str, name: &str) -> bool {
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

pub(super) async fn login(config: &Settings) -> Result<(Conn, String), String> {
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
