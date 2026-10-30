use alloc::collections::BTreeSet;
use core::num::{NonZeroU8, NonZeroU64};
use std::path::{Path, PathBuf};

use futures_util::future::join_all;
use serde::Deserialize;

use crate::ingest::{Profile, Source};
use crate::store::{Fallible, Shelf, connect};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    backoff_initial_ms: NonZeroU64,
    backoff_max_ms: NonZeroU64,
    busy_timeout_ms: NonZeroU64,
    failure_exit_code: NonZeroU8,
    manual_exit_code: NonZeroU8,
    sources: Vec<Source>,
    store_path: PathBuf,
}

impl Settings {
    pub(crate) const fn backoff_initial_ms(&self) -> u64 {
        self.backoff_initial_ms.get()
    }

    pub(crate) const fn backoff_max_ms(&self) -> u64 {
        self.backoff_max_ms.get()
    }

    pub(crate) const fn busy_timeout_ms(&self) -> u64 {
        self.busy_timeout_ms.get()
    }

    pub async fn check(&self) -> Result<(), String> {
        join_all(self.sources.iter().flat_map(Source::checks))
            .await
            .into_iter()
            .collect()
    }

    #[must_use = "退出码需交给进程"]
    pub const fn failure_exit_code(&self) -> u8 {
        self.failure_exit_code.get()
    }

    pub(crate) const fn manual_exit_code(&self) -> u8 {
        self.manual_exit_code.get()
    }

    #[must_use = "缺失的环境变量需报告"]
    pub fn missing(&self) -> Vec<&str> {
        self.sources.iter().flat_map(Source::missing).collect()
    }

    pub fn prepare(&self) -> Fallible<()> {
        connect(&self.store_path, self.busy_timeout_ms()).map(drop)
    }

    pub(crate) fn profile(&self, label: &str) -> Profile {
        self.sources
            .iter()
            .find(|source| source.label() == label)
            .map_or_default(Source::profile)
    }

    #[must_use = "货架需交给 Context"]
    pub fn shelf(&self) -> Shelf<'_> {
        Shelf::new(&self.store_path, self.busy_timeout_ms())
    }

    #[must_use = "数据源需逐个运行"]
    pub fn sources(&self) -> &[Source] {
        &self.sources
    }

    pub(crate) fn store_path(&self) -> &Path {
        &self.store_path
    }

    pub fn verify(&self) -> Result<(), String> {
        let labels: BTreeSet<String> = self.sources.iter().map(Source::label).collect();
        if self.sources.is_empty() {
            Err("ingest.sources: empty".to_owned())
        } else if self.sources.iter().any(Source::is_empty) {
            Err("ingest.sources: source without kind".to_owned())
        } else if labels.len() < self.sources.len() {
            Err("ingest.sources: duplicate source".to_owned())
        } else {
            Ok(())
        }
    }
}
