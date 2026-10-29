use alloc::collections::BTreeMap;
use core::num::{NonZeroU32, NonZeroU64};
use std::env;

use domain::review::Reason;
use serde::{Deserialize, Serialize};

use crate::pipe::probe;
use crate::seatbelt::Sandbox;

const THREAD: &str = "{thread}";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Approval {
    Auto,
    Manual,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exec {
    args: Vec<String>,
    bin: String,
    env: BTreeMap<String, String>,
    sandbox: Sandbox,
    timeout_ms: NonZeroU64,
    vars: BTreeMap<String, Var>,
}

impl Exec {
    pub(crate) fn args(&self) -> Vec<String> {
        self.args
            .iter()
            .map(|arg| {
                self.vars().fold(arg.clone(), |text, (name, value)| {
                    text.replace(&format!("${{{name}}}"), &value)
                })
            })
            .collect()
    }

    pub(crate) fn bin(&self) -> &str {
        &self.bin
    }

    pub(crate) const fn env(&self) -> &BTreeMap<String, String> {
        &self.env
    }

    pub(crate) const fn sandbox(&self) -> &Sandbox {
        &self.sandbox
    }

    pub(crate) const fn timeout_ms(&self) -> u64 {
        self.timeout_ms.get()
    }

    pub(crate) fn vars(&self) -> impl Iterator<Item = (&str, String)> {
        resolved(&self.vars)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Limits {
    batch: NonZeroU32,
    handoff_cooldown_ms: u64,
    intake: NonZeroU32,
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
pub struct Questions {
    threshold: f64,
    topic: Topic,
    urgent: Question,
}

impl Questions {
    pub(crate) const fn threshold(&self) -> f64 {
        self.threshold
    }

    pub(crate) const fn topic(&self) -> &Topic {
        &self.topic
    }

    pub(crate) const fn urgent(&self) -> &Question {
        &self.urgent
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Redact {
    patterns: Vec<Rule>,
    unknown_sender: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Review {
    learn_min_chars: NonZeroU32,
    mode: Approval,
    reasons: Vec<Reason>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Rule {
    pattern: String,
    placeholder: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Screen {
    exec: Exec,
    instructions: String,
    model: String,
    placeholder: String,
}

impl Screen {
    pub(crate) const fn exec(&self) -> &Exec {
        &self.exec
    }

    pub(crate) fn instructions(&self) -> &str {
        &self.instructions
    }

    pub(crate) fn model(&self) -> &str {
        &self.model
    }

    pub(crate) fn placeholder(&self) -> &str {
        &self.placeholder
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Session {
    followup: String,
    fresh: Vec<String>,
    prompt: String,
    resume: Vec<String>,
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
    questions: Questions,
    redact: Redact,
    review: Review,
    screen: Option<Screen>,
    self_sender_id: Var,
    session: Session,
    sizes: Sizes,
}

impl Settings {
    pub(crate) const fn batch(&self) -> u32 {
        self.limits.batch.get()
    }

    pub async fn check(&self) -> Result<(), String> {
        let screening = self.screen.iter().map(|screen| &screen.exec);
        for exec in [&self.decide, &self.codex, &self.notify]
            .into_iter()
            .chain(screening)
        {
            probe(exec).await?;
        }
        Ok(())
    }

    pub(crate) const fn codex(&self) -> &Exec {
        &self.codex
    }

    pub(crate) const fn context_max_chars(&self) -> u32 {
        self.sizes.context_max_chars.get()
    }

    pub(crate) const fn decide(&self) -> &Exec {
        &self.decide
    }

    #[must_use = "开关需用于决定是否运行"]
    pub fn enabled(&self) -> bool {
        self.mode == Mode::Enabled
    }

    pub(crate) fn followup(&self) -> &str {
        &self.session.followup
    }

    pub(crate) fn fresh_args(&self) -> Vec<String> {
        self.session.fresh.clone()
    }

    pub(crate) const fn handoff_cooldown_ms(&self) -> u64 {
        self.limits.handoff_cooldown_ms
    }

    pub(crate) const fn intake(&self) -> u32 {
        self.limits.intake.get()
    }

    pub(crate) const fn label_chars(&self) -> u32 {
        self.sizes.label_chars.get()
    }

    #[must_use = "下限需交给 Context"]
    pub const fn learn_min_chars(&self) -> u32 {
        self.review.learn_min_chars.get()
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
        .chain(self.codex.sandbox.vars())
        .chain(self.decide.sandbox.vars())
        .chain(self.notify.sandbox.vars())
        .chain(
            self.screen
                .iter()
                .flat_map(|screen| screen.exec.vars.values().chain(screen.exec.sandbox.vars())),
        )
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
        &self.session.prompt
    }

    pub(crate) const fn questions(&self) -> &Questions {
        &self.questions
    }

    #[must_use = "原因需交给 Context"]
    pub const fn reasons(&self) -> &[Reason] {
        self.review.reasons.as_slice()
    }

    pub(crate) const fn result_max_chars(&self) -> u32 {
        self.sizes.result_max_chars.get()
    }

    pub(crate) fn resume_args(&self, thread: &str) -> Vec<String> {
        self.session
            .resume
            .iter()
            .map(|arg| arg.replace(THREAD, thread))
            .collect()
    }

    pub(crate) const fn retry_after_ms(&self) -> u64 {
        self.limits.retry_after_ms
    }

    pub(crate) fn review_auto(&self) -> bool {
        self.review.mode == Approval::Auto
    }

    pub(crate) fn rules(&self) -> &[Rule] {
        &self.redact.patterns
    }

    pub(crate) const fn screen(&self) -> Option<&Screen> {
        self.screen.as_ref()
    }

    pub(crate) const fn self_sender_id(&self) -> &Var {
        &self.self_sender_id
    }

    pub(crate) const fn session_gap_ms(&self) -> u64 {
        self.sizes.session_gap_ms.get()
    }

    pub(crate) const fn session_max_ms(&self) -> u64 {
        self.sizes.session_max_ms.get()
    }

    pub(crate) const fn topics_max_open(&self) -> u32 {
        self.sizes.topics_max_open.get()
    }

    pub(crate) fn unknown_sender(&self) -> &str {
        &self.redact.unknown_sender
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Sizes {
    context_max_chars: NonZeroU32,
    label_chars: NonZeroU32,
    result_max_chars: NonZeroU32,
    session_gap_ms: NonZeroU64,
    session_max_ms: NonZeroU64,
    topics_max_open: NonZeroU32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Topic {
    instructions: String,
    min_confidence: f64,
    name: String,
    new_label: String,
}

impl Topic {
    pub(crate) fn instructions(&self) -> &str {
        &self.instructions
    }

    pub(crate) const fn min_confidence(&self) -> f64 {
        self.min_confidence
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn new_label(&self) -> &str {
        &self.new_label
    }
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

pub(crate) fn resolved(vars: &BTreeMap<String, Var>) -> impl Iterator<Item = (&str, String)> {
    vars.iter()
        .filter_map(|(name, var)| var.value().map(|value| (name.as_str(), value)))
}
