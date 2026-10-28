use alloc::collections::BTreeMap;
use core::num::{NonZeroU8, NonZeroU64};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::store::Shelf;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Dws {
    env: BTreeMap<String, String>,
    listen: Process,
    manual_exit_codes: Vec<i32>,
    pull: Process,
}

impl Dws {
    pub(crate) const fn env(&self) -> &BTreeMap<String, String> {
        &self.env
    }

    pub(crate) const fn listen(&self) -> &Process {
        &self.listen
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
    dws: Dws,
    failure_exit_code: NonZeroU8,
    initial_backfill_ms: u64,
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

    pub(crate) const fn dws(&self) -> &Dws {
        &self.dws
    }

    pub(crate) const fn failure_exit_code(&self) -> u8 {
        self.failure_exit_code.get()
    }

    pub(crate) const fn initial_backfill_ms(&self) -> u64 {
        self.initial_backfill_ms
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
