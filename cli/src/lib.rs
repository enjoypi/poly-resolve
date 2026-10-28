extern crate alloc;

mod args;
mod config;
mod http;
mod log;
mod spec;

use alloc::sync::Arc;
use core::fmt::Display;
use std::ffi::OsString;
use std::io::{self, Write as _};
use std::process::ExitCode;

use clap::Parser as _;
use domain::features::{self, Call};
use tokio::net::TcpListener;
use tracing::dispatcher::with_default;

use crate::args::Command;
use crate::config::Settings;

const CONFIG_ERROR: u8 = 2;

fn call(settings: &Settings, call: &Call<'_>) -> ExitCode {
    let result = with_default(settings.log(), || {
        features::dispatch(settings.errors(), call)
    });
    let (code, body) = result.map_or_else(
        |err| {
            (
                err.exit_code(settings.errors()),
                err.body(settings.errors()),
            )
        },
        |body| (0, body),
    );
    emit(code, &write_stdout(&body))
}

fn emit(code: u8, written: &io::Result<()>) -> ExitCode {
    ExitCode::from(code.max(u8::from(written.is_err())))
}

async fn execute(command: Command) -> ExitCode {
    match command {
        Command::Call(invocation) => match config::load(invocation.config()) {
            Ok(settings) => call(&settings, &invocation.call()),
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
    match TcpListener::bind(settings.addr()).await {
        Ok(listener) => {
            let (code, result) = http::run(listener, Arc::new(settings)).await;
            emit(code, &result)
        }
        Err(err) => fail(settings.serve_exit_code(), &err),
    }
}

fn write_stdout(body: &str) -> io::Result<()> {
    writeln!(io::stdout().lock(), "{body}")
}
