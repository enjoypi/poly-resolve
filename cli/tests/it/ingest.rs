use std::fs;
use std::io::{self, BufRead as _, BufReader};
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};

use crate::support::{YAML, await_log, config, run, terminate};

const EVENT: &str = r#"{"message_id":"m1","conversation_id":"c1","sender":"alice","sender_open_dingtalk_id":"u1","content":"hello","create_time":"2026-07-08 12:00:35"}"#;
const FAKE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../adapters/tests/it/fixtures/fake-dws.sh"
);

#[test]
fn ingests_until_signal() {
    let dir = sandbox("cli-ingest-signal").unwrap();
    fs::write(format!("{dir}/listen.txt"), format!("{EVENT}\n")).unwrap();
    let yaml = YAML
        .replace("/nonexistent/dws", FAKE)
        .replace("FAKE_DIR: /nonexistent", &format!("FAKE_DIR: {dir}"))
        .replace("/nonexistent/dir/store.db", &format!("{dir}/store.db"));
    let path = config("cli-ingest.yaml", &yaml).unwrap();
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

fn sandbox(name: &str) -> io::Result<String> {
    let dir = format!("{}/{name}", env!("CARGO_TARGET_TMPDIR"));
    if Path::new(&dir).exists() {
        fs::remove_dir_all(&dir)?;
    }
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[tokio::test]
async fn store_failure_uses_configured_exit_code() {
    let path = config("cli-ingest-store.yaml", YAML).unwrap();
    let code = run(&["cli", "ingest", "--config", &path]).await;
    assert_eq!(code, ExitCode::from(16));
}
