use std::ffi::OsString;
use std::fs;
use std::io::{self, BufReader, Lines};
use std::process::{Child, ChildStderr, Command, ExitCode, Output};

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
ingest:
  account: acct
  backoff_initial_ms: 5
  backoff_max_ms: 20
  child_exit_timeout_ms: 300
  dws:
    env:
      FAKE_DIR: /nonexistent
    listen:
      args: [listen]
      bin: /nonexistent/dws
    manual_exit_codes: [2, 4]
    pull:
      args: [pull]
      bin: /nonexistent/dws
  failure_exit_code: 16
  initial_backfill_ms: 3600000
  manual_exit_code: 17
  reconcile_overlap_ms: 1000
  source: dingtalk
  source_utc_offset_minutes: 480
  store_path: /nonexistent/dir/store.db
";

pub(crate) type Stderr = Lines<BufReader<ChildStderr>>;

pub(crate) fn await_log(lines: &mut Stderr, needle: &str) -> io::Result<()> {
    for line in lines {
        if line?.contains(needle) {
            return Ok(());
        }
    }
    Err(io::Error::other(format!("log not found: {needle}")))
}

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

pub(crate) fn terminate(child: &Child, signal: &str) -> io::Result<()> {
    let status = Command::new("kill")
        .args([signal, &child.id().to_string()])
        .status()?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| io::Error::other("kill failed"))
}
