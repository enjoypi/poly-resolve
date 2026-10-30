extern crate alloc;

mod args;
mod assets;
mod config;
mod http;
mod log;
mod service;
mod signal;
mod spec;

use alloc::sync::Arc;
use core::fmt::Display;
use std::ffi::OsString;
use std::io::{self, Write as _};
use std::process::ExitCode;

use adapters::triage::{Setup, validate};
use clap::Parser as _;
use tokio::net::TcpListener;

use crate::args::Command;
use crate::config::Settings;

const CONFIG_ERROR: u8 = 2;

fn emit(code: u8, written: &io::Result<()>) -> ExitCode {
    ExitCode::from(code.max(u8::from(written.is_err())))
}

async fn execute(command: Command) -> ExitCode {
    match command {
        Command::Ingest { config } => match config::load(&config) {
            Ok(settings) => ingest(settings).await,
            Err(err) => fail(CONFIG_ERROR, &err),
        },
        Command::Serve { config } => match config::load(&config) {
            Ok(settings) => serve(settings).await,
            Err(err) => fail(CONFIG_ERROR, &err),
        },
        Command::Spec => emit(0, &write_stdout(&spec::render())),
    }
}

fn fail(code: u8, err: &dyn Display) -> ExitCode {
    emit(code, &writeln!(io::stderr().lock(), "{err}"))
}

async fn ingest(settings: Settings) -> ExitCode {
    if let Err(err) = verify(&settings).await {
        return fail(settings.sandbox_exit_code(), &err);
    }
    let code = service::collect(Arc::new(settings)).await;
    emit(code, &Ok(()))
}

pub async fn run(args: Vec<OsString>) -> ExitCode {
    match Command::try_parse_from(args) {
        Ok(command) => execute(command).await,
        Err(err) => emit(
            u8::try_from(err.exit_code()).unwrap_or(CONFIG_ERROR),
            &err.print(),
        ),
    }
}

async fn serve(settings: Settings) -> ExitCode {
    let missing: Vec<&str> = settings
        .triage()
        .missing()
        .into_iter()
        .chain(settings.ingest().missing())
        .collect();
    if settings.triage().enabled() && !missing.is_empty() {
        let names = missing.join(", ");
        return fail(
            CONFIG_ERROR,
            &format!("triage: missing environment variables: {names}"),
        );
    }
    if let Err(err) = verify(&settings).await {
        return fail(settings.sandbox_exit_code(), &err);
    }
    match TcpListener::bind(settings.addr()).await {
        Ok(listener) => {
            let (code, result) = service::run(listener, Arc::new(settings)).await;
            emit(code, &result)
        }
        Err(err) => fail(settings.serve_exit_code(), &err),
    }
}

async fn verify(settings: &Settings) -> Result<(), String> {
    settings.ingest().check().await?;
    if settings.triage().enabled() {
        settings.triage().check().await?;
        let shelf = settings.ingest().shelf();
        let context = settings.context(&shelf);
        validate(Setup::new(settings.ingest(), settings.triage(), &context))
    } else {
        Ok(())
    }
}

fn write_stdout(body: &str) -> io::Result<()> {
    writeln!(io::stdout().lock(), "{body}")
}
