use std::ffi::OsString;
use std::fs;
use std::io;
use std::process::{Command, ExitCode, Output};

use cli::run as run_cli;

pub(crate) const YAML: &str = "addr: \"127.0.0.1:0\"
log_level: debug
serve_exit_code: 9
shutdown_timeout_exit_code: 8
shutdown_timeout_ms: 10000
errors:
  bad_request:
    code: invalid
    exit_code: 13
  unknown_action:
    code: missing
    exit_code: 14
";

pub(crate) async fn call(config: &str, target: [&str; 2], json: &str) -> ExitCode {
    let [feature, action] = target;
    run(&[
        "cli", "call", feature, action, "--config", config, "--json", json,
    ])
    .await
}

pub(crate) fn config(name: &str, body: &str) -> io::Result<String> {
    let path = format!("{}/{name}", env!("CARGO_TARGET_TMPDIR"));
    fs::write(&path, body)?;
    Ok(path)
}

pub(crate) async fn run(args: &[&str]) -> ExitCode {
    run_cli(args.iter().map(OsString::from).collect()).await
}

pub(crate) fn spec() -> io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_cli")).arg("spec").output()
}
