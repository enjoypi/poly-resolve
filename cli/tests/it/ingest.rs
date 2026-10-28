use std::fs;
use std::io::{BufRead as _, BufReader};
use std::process::{Command, ExitCode, Stdio};

use crate::support::{EVENT, TRIAGE, await_log, config, live, run, sandbox, terminate};

fn absent(dir: &str) -> String {
    live(dir).replace("fixtures/fake.sb", "fixtures/absent.sb")
}

#[tokio::test]
async fn ingest_exits_when_sandbox_fails() {
    let dir = sandbox("cli-ingest-sandbox").unwrap();
    let path = config("cli-ingest-sandbox.yaml", &absent(&dir)).unwrap();
    let code = run(&["cli", "ingest", "--config", &path]).await;
    assert_eq!(code, ExitCode::from(18));
}

#[test]
fn ingests_until_signal() {
    let dir = sandbox("cli-ingest-signal").unwrap();
    fs::write(format!("{dir}/listen.txt"), format!("{EVENT}\n")).unwrap();
    let path = config("cli-ingest.yaml", &live(&dir)).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_cli"))
        .args(["ingest", "--config", &path])
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stderr = child.stderr.take().unwrap();
    let mut lines = BufReader::new(stderr).lines();
    await_log(&mut lines, "event stored").unwrap();
    terminate(&child, "-TERM").unwrap();
    assert!(child.wait().unwrap().success());
}

#[tokio::test]
async fn missing_config_fails() {
    let code = run(&["cli", "ingest", "--config", "/nonexistent/config.yaml"]).await;
    assert_eq!(code, ExitCode::from(2));
}

#[tokio::test]
async fn serve_exits_when_sandbox_fails() {
    let dir = sandbox("cli-serve-sandbox").unwrap();
    let path = config("cli-serve-sandbox.yaml", &absent(&dir)).unwrap();
    let code = run(&["cli", "serve", "--config", &path]).await;
    assert_eq!(code, ExitCode::from(18));
}

#[tokio::test]
async fn serve_exits_when_triage_sandbox_fails() {
    let dir = sandbox("cli-serve-triage-sandbox").unwrap();
    let yaml = live(&dir)
        .replace("mode: disabled", "mode: enabled")
        .replace(
            &format!("decide: {{args: [x], bin: {TRIAGE}"),
            "decide: {args: [x], bin: /nonexistent/triage",
        );
    let path = config("cli-serve-triage-sandbox.yaml", &yaml).unwrap();
    let code = run(&["cli", "serve", "--config", &path]).await;
    assert_eq!(code, ExitCode::from(18));
}

#[tokio::test]
async fn serve_stops_when_collection_fails() {
    let dir = sandbox("cli-serve-store").unwrap();
    let path = config("cli-serve-store.yaml", &unwritable(&dir)).unwrap();
    let code = run(&["cli", "serve", "--config", &path]).await;
    assert_eq!(code, ExitCode::from(16));
}

#[tokio::test]
async fn store_failure_uses_configured_exit_code() {
    let dir = sandbox("cli-ingest-store").unwrap();
    let path = config("cli-ingest-store.yaml", &unwritable(&dir)).unwrap();
    let code = run(&["cli", "ingest", "--config", &path]).await;
    assert_eq!(code, ExitCode::from(16));
}

fn unwritable(dir: &str) -> String {
    live(dir).replace(&format!("{dir}/store.db"), "/nonexistent/dir/store.db")
}
