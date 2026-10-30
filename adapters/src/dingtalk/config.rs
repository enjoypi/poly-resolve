use alloc::collections::BTreeMap;
use core::num::{NonZeroU32, NonZeroU64};

use serde::Deserialize;

use super::dws::Dws as Runner;
use crate::seatbelt::Sandbox;
use crate::triage::config::{Var, resolved};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Dws {
    decrypt: Process,
    env: BTreeMap<String, String>,
    listen: Process,
    lookup: Process,
    manual_exit_codes: Vec<i32>,
    parallel: NonZeroU32,
    pull: Process,
    timeout_ms: NonZeroU64,
    vars: BTreeMap<String, Var>,
}

impl Dws {
    pub(crate) const fn decrypt(&self) -> &Process {
        &self.decrypt
    }

    pub(crate) const fn env(&self) -> &BTreeMap<String, String> {
        &self.env
    }

    pub(crate) const fn listen(&self) -> &Process {
        &self.listen
    }

    pub(crate) const fn lookup(&self) -> &Process {
        &self.lookup
    }

    pub(crate) fn manual_exit_codes(&self) -> &[i32] {
        &self.manual_exit_codes
    }

    pub(crate) fn parallel(&self) -> usize {
        usize::try_from(self.parallel.get()).unwrap_or(usize::MAX)
    }

    pub(crate) const fn pull(&self) -> &Process {
        &self.pull
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
pub(crate) struct Process {
    args: Vec<String>,
    bin: String,
    sandbox: Sandbox,
}

impl Process {
    pub(crate) fn args(&self) -> &[String] {
        &self.args
    }

    pub(crate) fn bin(&self) -> &str {
        &self.bin
    }

    pub(crate) const fn sandbox(&self) -> &Sandbox {
        &self.sandbox
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Settings {
    account: String,
    child_exit_timeout_ms: NonZeroU64,
    cipher_min_length: NonZeroU32,
    dws: Dws,
    heal_limit: NonZeroU32,
    initial_backfill_ms: u64,
    lookup_batch: NonZeroU32,
    notify_conversation_id: Var,
    reconcile_overlap_ms: u64,
    self_sender_id: Var,
    utc_offset_minutes: i16,
}

impl Settings {
    pub(crate) fn account(&self) -> &str {
        &self.account
    }

    pub(crate) async fn check(&self) -> Result<(), String> {
        Runner::new(&self.dws).check().await
    }

    pub(crate) const fn child_exit_timeout_ms(&self) -> u64 {
        self.child_exit_timeout_ms.get()
    }

    pub(crate) const fn cipher_min_length(&self) -> u32 {
        self.cipher_min_length.get()
    }

    pub(crate) const fn dws(&self) -> &Dws {
        &self.dws
    }

    pub(crate) const fn heal_limit(&self) -> u32 {
        self.heal_limit.get()
    }

    pub(crate) const fn initial_backfill_ms(&self) -> u64 {
        self.initial_backfill_ms
    }

    pub(crate) fn lookup_batch(&self) -> usize {
        usize::try_from(self.lookup_batch.get()).unwrap_or(usize::MAX)
    }

    pub(crate) const fn notify_conversation_id(&self) -> &Var {
        &self.notify_conversation_id
    }

    pub(crate) const fn reconcile_overlap_ms(&self) -> u64 {
        self.reconcile_overlap_ms
    }

    pub(crate) const fn self_sender_id(&self) -> &Var {
        &self.self_sender_id
    }

    pub(crate) const fn utc_offset_minutes(&self) -> i16 {
        self.utc_offset_minutes
    }

    pub(crate) const fn vars(&self) -> [&Var; 2] {
        [&self.notify_conversation_id, &self.self_sender_id]
    }
}
