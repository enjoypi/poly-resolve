use std::time::{SystemTime, UNIX_EPOCH};

use domain::features::Context;
use serde::Deserialize;

use crate::config::Settings;
use crate::dingtalk::config::Settings as Dingtalk;
use crate::dingtalk::{SOURCE as DINGTALK, run as dingtalk};
use crate::race::Stop;
use crate::triage::config::Var;

#[derive(Debug, Default)]
pub(crate) struct Profile {
    cipher_min_length: Option<u32>,
    notify_conversation_id: Option<String>,
    self_sender_id: Option<String>,
}

impl Profile {
    pub(crate) const fn cipher_min_length(&self) -> Option<u32> {
        self.cipher_min_length
    }

    pub(crate) fn notify_conversation_id(&self) -> Option<&str> {
        self.notify_conversation_id.as_deref()
    }

    pub(crate) fn self_sender_id(&self) -> Option<&str> {
        self.self_sender_id.as_deref()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Site<'site> {
    context: &'site Context<'site>,
    settings: &'site Settings,
}

impl<'site> Site<'site> {
    pub(crate) const fn context(self) -> &'site Context<'site> {
        self.context
    }

    #[must_use = "现场需交给 run"]
    pub const fn new(settings: &'site Settings, context: &'site Context<'site>) -> Self {
        Self { context, settings }
    }

    pub(crate) const fn settings(self) -> &'site Settings {
        self.settings
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    dingtalk: Option<Dingtalk>,
}

impl Source {
    pub(crate) fn checks(&self) -> impl Iterator<Item = impl Future<Output = Result<(), String>>> {
        self.dingtalk.iter().map(Dingtalk::check)
    }

    pub(crate) const fn is_empty(&self) -> bool {
        self.dingtalk.is_none()
    }

    #[must_use = "标签用于日志与去重"]
    pub fn label(&self) -> String {
        self.dingtalk
            .as_ref()
            .map_or_default(|config| label(DINGTALK, config.account()))
    }

    pub(crate) fn missing(&self) -> Vec<&str> {
        self.dingtalk
            .iter()
            .flat_map(Dingtalk::vars)
            .filter(|var| var.value().is_none())
            .map(Var::name)
            .collect()
    }

    pub(crate) fn profile(&self) -> Profile {
        self.dingtalk.as_ref().map_or_default(|config| Profile {
            cipher_min_length: Some(config.cipher_min_length()),
            notify_conversation_id: config.notify_conversation_id().value(),
            self_sender_id: config.self_sender_id().value(),
        })
    }
}

pub(crate) fn label(source: &str, account: &str) -> String {
    format!("{source}:{account}")
}

pub(crate) fn now_ms() -> u64 {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
}

pub async fn run(site: Site<'_>, source: &Source, stop: Stop<'_>) -> u8 {
    match source.dingtalk.as_ref() {
        Some(config) => dingtalk(site, config, stop).await,
        None => 0,
    }
}
