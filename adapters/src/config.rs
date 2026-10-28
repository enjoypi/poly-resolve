use alloc::collections::BTreeMap;
use core::num::{NonZeroU8, NonZeroU32, NonZeroU64};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::store::Shelf;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Dws {
    decrypt: Process,
    env: BTreeMap<String, String>,
    listen: Process,
    lookup: Process,
    manual_exit_codes: Vec<i32>,
    pull: Process,
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

    pub(crate) const fn pull(&self) -> &Process {
        &self.pull
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Process {
    args: Vec<String>,
    bin: String,
}

impl Process {
    pub(crate) fn args(&self) -> &[String] {
        &self.args
    }

    pub(crate) fn bin(&self) -> &str {
        &self.bin
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    account: String,
    backoff_initial_ms: NonZeroU64,
    backoff_max_ms: NonZeroU64,
    child_exit_timeout_ms: NonZeroU64,
    cipher_min_length: NonZeroU32,
    dws: Dws,
    failure_exit_code: NonZeroU8,
    heal_limit: NonZeroU32,
    initial_backfill_ms: u64,
    lookup_batch: NonZeroU32,
    manual_exit_code: NonZeroU8,
    reconcile_overlap_ms: u64,
    source: String,
    source_utc_offset_minutes: i16,
    store_path: PathBuf,
}

impl Settings {
    pub(crate) fn account(&self) -> &str {
        &self.account
    }

    pub(crate) const fn backoff_initial_ms(&self) -> u64 {
        self.backoff_initial_ms.get()
    }

    pub(crate) const fn backoff_max_ms(&self) -> u64 {
        self.backoff_max_ms.get()
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

    pub(crate) const fn failure_exit_code(&self) -> u8 {
        self.failure_exit_code.get()
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

    pub(crate) const fn manual_exit_code(&self) -> u8 {
        self.manual_exit_code.get()
    }

    pub(crate) const fn reconcile_overlap_ms(&self) -> u64 {
        self.reconcile_overlap_ms
    }

    #[must_use = "货架需交给 Context"]
    pub fn shelf(&self) -> Shelf<'_> {
        Shelf::new(&self.store_path, &self.source, &self.account)
    }

    pub(crate) fn source(&self) -> &str {
        &self.source
    }

    pub(crate) const fn source_utc_offset_minutes(&self) -> i16 {
        self.source_utc_offset_minutes
    }

    pub(crate) fn store_path(&self) -> &Path {
        &self.store_path
    }
}
