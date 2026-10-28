use std::fs;
use std::io::{BufRead as _, BufReader};
use std::process::{Command, ExitCode, Stdio};

use crate::support::{EVENT, YAML, await_log, config, live, run, sandbox, terminate};

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
async fn serve_stops_when_collection_fails() {
    let path = config("cli-serve-store.yaml", YAML).unwrap();
    let code = run(&["cli", "serve", "--config", &path]).await;
    assert_eq!(code, ExitCode::from(16));
}

#[tokio::test]
async fn store_failure_uses_configured_exit_code() {
    let path = config("cli-ingest-store.yaml", YAML).unwrap();
    let code = run(&["cli", "ingest", "--config", &path]).await;
    assert_eq!(code, ExitCode::from(16));
}
