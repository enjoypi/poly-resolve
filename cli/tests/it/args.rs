use std::process::{Command, ExitCode};

use crate::support::{YAML, call, config, run};

#[tokio::test]
async fn call_returns_business_exit_code() {
    let path = config("cli-call.yaml", YAML).unwrap();
    assert_eq!(
        call(&path, ["health", "check"], "{}").await,
        ExitCode::SUCCESS
    );
    assert_eq!(
        call(&path, ["health", "check"], "nope").await,
        ExitCode::from(13)
    );
    assert_eq!(
        call(&path, ["nope", "nope"], "{}").await,
        ExitCode::from(14)
    );
}

#[test]
fn call_writes_response_to_stdout() {
    let path = config("cli-stdout.yaml", YAML).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_cli"))
        .args([
            "call",
            "health",
            "check",
            "--json",
            "{}",
            "--session-id",
            "s1",
            "--user",
            "u1",
            "--config",
        ])
        .arg(path)
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "{\"status\":\"ok\"}\n"
    );
    let log = String::from_utf8(out.stderr).unwrap();
    assert!(log.contains(r#""target":"dispatch""#));
    assert!(log.contains(r#""session_id":"s1""#));
}

#[tokio::test]
async fn help_and_bad_args_follow_clap_codes() {
    assert_eq!(run(&["cli", "--help"]).await, ExitCode::SUCCESS);
    assert_eq!(run(&["cli", "bogus"]).await, ExitCode::from(2));
    assert_eq!(run(&["cli"]).await, ExitCode::from(2));
}
