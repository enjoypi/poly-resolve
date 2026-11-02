use core::net::IpAddr;
use core::num::{NonZeroU32, NonZeroU64};

use http::Uri;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::net::{Layer, proxied};
use crate::triage::config::Var;
use crate::triage::endpoint::Location;

const PLAIN: &str = "imap";
const PLAIN_PORT: u16 = 143;
const TLS: &str = "imaps";
const TLS_PORT: u16 = 993;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Limits {
    chars: NonZeroU32,
    depth: NonZeroU32,
    fetch_batch: NonZeroU32,
    line_bytes: NonZeroU32,
    message_bytes: NonZeroU32,
    parts: NonZeroU32,
    search_max: NonZeroU32,
}

impl Limits {
    pub(crate) fn fetch_batch(&self) -> usize {
        usize::try_from(self.fetch_batch.get()).unwrap_or(usize::MAX)
    }

    pub(crate) fn line_bytes(&self) -> usize {
        usize::try_from(self.line_bytes.get()).unwrap_or(usize::MAX)
    }

    pub(crate) const fn message_bytes(&self) -> u32 {
        self.message_bytes.get()
    }

    pub(crate) fn mime(&self) -> Value {
        json!({
            "chars": self.chars.get(),
            "depth": self.depth.get(),
            "parts": self.parts.get(),
        })
    }

    pub(crate) fn search_max(&self) -> usize {
        usize::try_from(self.search_max.get()).unwrap_or(usize::MAX)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Settings {
    account: String,
    idle_ms: NonZeroU64,
    initial_backfill_ms: u64,
    limits: Limits,
    mailbox: String,
    password: Var,
    poll_ms: NonZeroU64,
    proxy: Option<Location>,
    reconcile_overlap_ms: u64,
    self_sender_ids: Vec<Var>,
    style: Value,
    timeout_ms: NonZeroU64,
    url: Location,
    user: Var,
}

impl Settings {
    pub(crate) fn account(&self) -> &str {
        &self.account
    }

    pub(crate) const fn idle_ms(&self) -> u64 {
        self.idle_ms.get()
    }

    pub(crate) const fn initial_backfill_ms(&self) -> u64 {
        self.initial_backfill_ms
    }

    pub(crate) const fn limits(&self) -> &Limits {
        &self.limits
    }

    pub(crate) fn mailbox(&self) -> &str {
        &self.mailbox
    }

    pub(crate) fn password(&self) -> String {
        self.password.value().unwrap_or_default()
    }

    pub(crate) const fn poll_ms(&self) -> u64 {
        self.poll_ms.get()
    }

    pub(crate) const fn reconcile_overlap_ms(&self) -> u64 {
        self.reconcile_overlap_ms
    }

    pub(crate) fn self_sender_ids(&self) -> Vec<String> {
        self.self_sender_ids.iter().filter_map(Var::value).collect()
    }

    pub(crate) const fn style(&self) -> &Value {
        &self.style
    }

    pub(crate) fn target(&self) -> Result<Target, String> {
        let url = self
            .url
            .value()
            .unwrap_or_default()
            .parse::<Uri>()
            .map_err(|_err| "imap:url".to_owned())?;
        let proxy = self
            .proxy
            .as_ref()
            .map(|found| proxied(&found.value().unwrap_or_default()))
            .transpose()?;
        let host = url.host().unwrap_or_default().to_owned();
        let literal = host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback());
        let (layer, port) = match url.scheme_str() {
            Some(TLS) if self.url.var().is_some() => (Layer::Tls, TLS_PORT),
            Some(PLAIN) if literal && proxy.is_none() => (Layer::Plain, PLAIN_PORT),
            _ => return Err("imap:url".to_owned()),
        };
        Ok(Target {
            host,
            layer,
            port: url.port_u16().unwrap_or(port),
            proxy,
        })
    }

    pub(crate) const fn timeout_ms(&self) -> u64 {
        self.timeout_ms.get()
    }

    pub(crate) fn user(&self) -> String {
        self.user.value().unwrap_or_default()
    }

    pub(crate) fn vars(&self) -> impl Iterator<Item = &Var> {
        [&self.user, &self.password]
            .into_iter()
            .chain(self.url.var())
            .chain(self.proxy.iter().filter_map(Location::var))
            .chain(self.self_sender_ids.iter())
    }
}

pub(crate) struct Target {
    host: String,
    layer: Layer,
    port: u16,
    proxy: Option<Uri>,
}

impl Target {
    pub(crate) fn host(&self) -> &str {
        &self.host
    }

    pub(crate) const fn layer(&self) -> Layer {
        self.layer
    }

    pub(crate) const fn port(&self) -> u16 {
        self.port
    }

    pub(crate) const fn proxy(&self) -> Option<&Uri> {
        self.proxy.as_ref()
    }
}
