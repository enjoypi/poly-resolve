pub(crate) mod cycle;

use core::pin::Pin;
use std::time::{SystemTime, UNIX_EPOCH};

use domain::features::Context;
use serde::Deserialize;

use crate::config::Settings;
use crate::dingtalk::config::Settings as Dingtalk;
use crate::dingtalk::{SOURCE as DINGTALK, run as dingtalk};
use crate::imap::config::Settings as Imap;
use crate::imap::{SOURCE as IMAP, check as imap_check, run as imap};
use crate::race::Stop;
use crate::triage::config::Var;

type Checking<'check> = Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'check>>;

#[derive(Debug, Default)]
pub(crate) struct Profile {
    cipher_min_length: Option<u32>,
    notify_conversation_id: Option<String>,
    self_sender_ids: Vec<String>,
}

impl Profile {
    pub(crate) const fn cipher_min_length(&self) -> Option<u32> {
        self.cipher_min_length
    }

    pub(crate) fn notify_conversation_id(&self) -> Option<&str> {
        self.notify_conversation_id.as_deref()
    }

    pub(crate) fn self_sender_ids(&self) -> &[String] {
        &self.self_sender_ids
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
    imap: Option<Imap>,
}

impl Source {
    pub(crate) fn checks(&self) -> impl Iterator<Item = Checking<'_>> {
        self.dingtalk
            .iter()
            .map(dingtalk_checked)
            .chain(self.imap.iter().map(imap_checked))
    }

    pub(crate) const fn is_empty(&self) -> bool {
        self.dingtalk.is_none() && self.imap.is_none()
    }

    #[must_use = "标签用于日志与去重"]
    pub fn label(&self) -> String {
        self.dingtalk
            .as_ref()
            .map(|config| label(DINGTALK, config.account()))
            .or_else(|| {
                self.imap
                    .as_ref()
                    .map(|config| label(IMAP, config.account()))
            })
            .unwrap_or_default()
    }

    pub(crate) fn missing(&self) -> Vec<&str> {
        self.dingtalk
            .iter()
            .flat_map(Dingtalk::vars)
            .chain(self.imap.iter().flat_map(Imap::vars))
            .filter(|var| var.value().is_none())
            .map(Var::name)
            .collect()
    }

    pub(crate) fn profile(&self) -> Profile {
        self.dingtalk
            .as_ref()
            .map(|config| Profile {
                cipher_min_length: Some(config.cipher_min_length()),
                notify_conversation_id: config.notify_conversation_id().value(),
                self_sender_ids: config.self_sender_id().value().into_iter().collect(),
            })
            .or_else(|| {
                self.imap.as_ref().map(|config| Profile {
                    cipher_min_length: None,
                    notify_conversation_id: None,
                    self_sender_ids: config.self_sender_ids(),
                })
            })
            .unwrap_or_default()
    }
}

fn dingtalk_checked(config: &Dingtalk) -> Checking<'_> {
    Box::pin(config.check())
}

fn imap_checked(config: &Imap) -> Checking<'_> {
    Box::pin(imap_check(config))
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
    match (source.dingtalk.as_ref(), source.imap.as_ref()) {
        (Some(config), _) => dingtalk(site, config, stop).await,
        (None, Some(config)) => imap(site, config, stop).await,
        (None, None) => 0,
    }
}
