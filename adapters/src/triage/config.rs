use alloc::collections::BTreeMap;
use core::num::{NonZeroU32, NonZeroU64};
use std::env;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exec {
    args: Vec<String>,
    bin: String,
    env: BTreeMap<String, String>,
    timeout_ms: NonZeroU64,
    vars: BTreeMap<String, Var>,
}

impl Exec {
    pub(crate) fn args(&self) -> &[String] {
        &self.args
    }

    pub(crate) fn bin(&self) -> &str {
        &self.bin
    }

    pub(crate) const fn env(&self) -> &BTreeMap<String, String> {
        &self.env
    }

    pub(crate) const fn timeout_ms(&self) -> u64 {
        self.timeout_ms.get()
    }

    pub(crate) fn vars(&self) -> impl Iterator<Item = (&str, String)> {
        self.vars
            .iter()
            .filter_map(|(name, var)| var.value().map(|value| (name.as_str(), value)))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Limits {
    batch: NonZeroU32,
    handoff_cooldown_ms: u64,
    lookback_ms: u64,
    max_attempts: NonZeroU32,
    max_decisions_per_hour: u32,
    max_handoffs_per_hour: u32,
    poll_ms: NonZeroU64,
    retry_after_ms: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Mode {
    Disabled,
    Enabled,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Question {
    instructions: String,
    name: String,
}

impl Question {
    pub(crate) fn instructions(&self) -> &str {
        &self.instructions
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    codex: Exec,
    decide: Exec,
    limits: Limits,
    mode: Mode,
    model: String,
    notice: String,
    notify: Exec,
    notify_conversation_id: Var,
    notify_target: Var,
    prompt: String,
    question: Question,
    self_sender_id: Var,
    sizes: Sizes,
    threshold: f64,
}

impl Settings {
    pub(crate) const fn batch(&self) -> u32 {
        self.limits.batch.get()
    }

    pub(crate) const fn codex(&self) -> &Exec {
        &self.codex
    }

    pub(crate) const fn context_max_chars(&self) -> u32 {
        self.sizes.context_max_chars.get()
    }

    pub(crate) const fn context_messages(&self) -> u32 {
        self.sizes.context_messages.get()
    }

    pub(crate) const fn context_window_ms(&self) -> u64 {
        self.sizes.context_window_ms
    }

    pub(crate) const fn decide(&self) -> &Exec {
        &self.decide
    }

    #[must_use = "开关需用于决定是否运行"]
    pub fn enabled(&self) -> bool {
        self.mode == Mode::Enabled
    }

    pub(crate) const fn handoff_cooldown_ms(&self) -> u64 {
        self.limits.handoff_cooldown_ms
    }

    pub(crate) const fn input_max_chars(&self) -> u32 {
        self.sizes.input_max_chars.get()
    }

    pub(crate) const fn lookback_ms(&self) -> u64 {
        self.limits.lookback_ms
    }

    pub(crate) const fn max_attempts(&self) -> u32 {
        self.limits.max_attempts.get()
    }

    pub(crate) const fn max_decisions_per_hour(&self) -> u32 {
        self.limits.max_decisions_per_hour
    }

    pub(crate) const fn max_handoffs_per_hour(&self) -> u32 {
        self.limits.max_handoffs_per_hour
    }

    #[must_use = "缺失的环境变量需报告"]
    pub fn missing(&self) -> Vec<&str> {
        [
            &self.notify_conversation_id,
            &self.notify_target,
            &self.self_sender_id,
        ]
        .into_iter()
        .chain(self.codex.vars.values())
        .chain(self.decide.vars.values())
        .chain(self.notify.vars.values())
        .filter(|var| var.value().is_none())
        .map(|var| var.name.as_str())
        .collect()
    }

    pub(crate) fn model(&self) -> &str {
        &self.model
    }

    pub(crate) fn notice(&self) -> &str {
        &self.notice
    }

    pub(crate) const fn notify(&self) -> &Exec {
        &self.notify
    }

    pub(crate) const fn notify_conversation_id(&self) -> &Var {
        &self.notify_conversation_id
    }

    pub(crate) const fn notify_target(&self) -> &Var {
        &self.notify_target
    }

    pub(crate) const fn poll_ms(&self) -> u64 {
        self.limits.poll_ms.get()
    }

    pub(crate) fn prompt(&self) -> &str {
        &self.prompt
    }

    pub(crate) const fn question(&self) -> &Question {
        &self.question
    }

    pub(crate) const fn result_max_chars(&self) -> u32 {
        self.sizes.result_max_chars.get()
    }

    pub(crate) const fn retry_after_ms(&self) -> u64 {
        self.limits.retry_after_ms
    }

    pub(crate) const fn self_sender_id(&self) -> &Var {
        &self.self_sender_id
    }

    pub(crate) const fn threshold(&self) -> f64 {
        self.threshold
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Sizes {
    context_max_chars: NonZeroU32,
    context_messages: NonZeroU32,
    context_window_ms: u64,
    input_max_chars: NonZeroU32,
    result_max_chars: NonZeroU32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(try_from = "String")]
pub struct Var {
    name: String,
}

impl Var {
    pub(crate) fn value(&self) -> Option<String> {
        env::var(&self.name).ok().filter(|found| !found.is_empty())
    }
}

impl TryFrom<String> for Var {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let name = value
            .strip_prefix("${")
            .and_then(|rest| rest.strip_suffix('}'))
            .filter(|inner| !inner.is_empty())
            .ok_or_else(|| "expected ${NAME}".to_owned())?;
        Ok(Self {
            name: name.to_owned(),
        })
    }
}
