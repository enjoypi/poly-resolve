use core::net::SocketAddr;
use core::num::{NonZeroU8, NonZeroU64};
use std::fs;
use std::path::{Path, PathBuf};

use adapters::config::Settings as Ingest;
use adapters::triage::config::Settings as Triage;
use domain::Errors;
use domain::features::Context;
use domain::inbox::{Paging, Reader};
use serde::Deserialize;
use serde_saphyr::from_str;
use tracing::Dispatch;
use tracing_subscriber::filter::LevelFilter;

use crate::log;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    addr: SocketAddr,
    errors: Errors,
    ingest: Ingest,
    log_level: String,
    messages: Paging,
    serve_exit_code: NonZeroU8,
    shutdown_timeout_exit_code: NonZeroU8,
    shutdown_timeout_ms: NonZeroU64,
    triage: Triage,
    web_dir: PathBuf,
}

pub(crate) struct Settings {
    addr: SocketAddr,
    errors: Errors,
    ingest: Ingest,
    log: Dispatch,
    messages: Paging,
    serve_exit_code: u8,
    shutdown_timeout_exit_code: u8,
    shutdown_timeout_ms: u64,
    triage: Triage,
    web_dir: PathBuf,
}

impl Settings {
    pub(crate) const fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub(crate) const fn context<'ctx>(&'ctx self, reader: &'ctx dyn Reader) -> Context<'ctx> {
        Context::new(&self.errors, &self.messages, reader)
    }

    pub(crate) const fn errors(&self) -> &Errors {
        &self.errors
    }

    pub(crate) const fn ingest(&self) -> &Ingest {
        &self.ingest
    }

    pub(crate) const fn log(&self) -> &Dispatch {
        &self.log
    }

    pub(crate) const fn serve_exit_code(&self) -> u8 {
        self.serve_exit_code
    }

    pub(crate) const fn shutdown_timeout_exit_code(&self) -> u8 {
        self.shutdown_timeout_exit_code
    }

    pub(crate) const fn shutdown_timeout_ms(&self) -> u64 {
        self.shutdown_timeout_ms
    }

    pub(crate) const fn triage(&self) -> &Triage {
        &self.triage
    }

    pub(crate) fn web_dir(&self) -> &Path {
        &self.web_dir
    }
}

pub(crate) fn load(path: &Path) -> Result<Settings, String> {
    let text = fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
    let file: File = from_str(&text).map_err(|err| format!("{}: {err}", path.display()))?;
    let level = file
        .log_level
        .parse::<LevelFilter>()
        .map_err(|err| format!("{}: log_level: {err}", path.display()))?;
    if !file.addr.ip().is_loopback() {
        return Err(format!(
            "{}: addr: {} is not loopback",
            path.display(),
            file.addr
        ));
    }
    Ok(Settings {
        addr: file.addr,
        errors: file.errors,
        ingest: file.ingest,
        log: log::dispatch(level),
        messages: file.messages,
        serve_exit_code: file.serve_exit_code.get(),
        shutdown_timeout_exit_code: file.shutdown_timeout_exit_code.get(),
        shutdown_timeout_ms: file.shutdown_timeout_ms.get(),
        triage: file.triage,
        web_dir: file.web_dir,
    })
}
