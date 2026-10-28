use std::process::ExitCode;

use crate::support::{YAML, call, config, run};

#[tokio::test]
async fn bad_addr_fails() {
    let path = config("bad-addr.yaml", &YAML.replace("127.0.0.1:0", "localhost:0")).unwrap();
    assert_eq!(
        call(&path, ["health", "check"], "{}").await,
        ExitCode::from(2)
    );
}

#[tokio::test]
async fn bad_level_fails() {
    let path = config("bad-level.yaml", &YAML.replace("debug", "loud")).unwrap();
    assert_eq!(
        call(&path, ["health", "check"], "{}").await,
        ExitCode::from(2)
    );
}

#[tokio::test]
async fn bad_yaml_fails() {
    let path = config("bad-yaml.yaml", "addr: [\n").unwrap();
    assert_eq!(
        call(&path, ["health", "check"], "{}").await,
        ExitCode::from(2)
    );
}

#[tokio::test]
async fn ipv6_loopback_addr_loads() {
    let path = config("ipv6.yaml", &YAML.replace("127.0.0.1:0", "[::1]:0")).unwrap();
    assert_eq!(
        call(&path, ["health", "check"], "{}").await,
        ExitCode::SUCCESS
    );
}

#[tokio::test]
async fn missing_file_fails() {
    let code = call("/nonexistent/config.yaml", ["health", "check"], "{}").await;
    assert_eq!(code, ExitCode::from(2));
}

#[tokio::test]
async fn non_loopback_addr_fails() {
    for (name, addr) in [
        ("non-loopback.yaml", "0.0.0.0:0"),
        ("non-loopback-v6.yaml", "[::]:0"),
    ] {
        let path = config(name, &YAML.replace("127.0.0.1:0", addr)).unwrap();
        assert_eq!(
            call(&path, ["health", "check"], "{}").await,
            ExitCode::from(2)
        );
    }
}

#[tokio::test]
async fn serve_with_missing_file_fails() {
    let code = run(&["cli", "serve", "--config", "/nonexistent/config.yaml"]).await;
    assert_eq!(code, ExitCode::from(2));
}

#[tokio::test]
async fn unknown_field_fails() {
    let path = config("unknown-field.yaml", &format!("{YAML}extra: 1\n")).unwrap();
    assert_eq!(
        call(&path, ["health", "check"], "{}").await,
        ExitCode::from(2)
    );
}

#[tokio::test]
async fn zero_exit_code_fails() {
    let path = config("zero-exit.yaml", &YAML.replace('9', "0")).unwrap();
    assert_eq!(
        call(&path, ["health", "check"], "{}").await,
        ExitCode::from(2)
    );
}

#[tokio::test]
async fn zero_shutdown_timeout_exit_code_fails() {
    let yaml = YAML.replace(
        "shutdown_timeout_exit_code: 8",
        "shutdown_timeout_exit_code: 0",
    );
    let path = config("zero-timeout-exit.yaml", &yaml).unwrap();
    assert_eq!(
        call(&path, ["health", "check"], "{}").await,
        ExitCode::from(2)
    );
}

#[tokio::test]
async fn zero_shutdown_timeout_fails() {
    let yaml = YAML.replace("shutdown_timeout_ms: 10000", "shutdown_timeout_ms: 0");
    let path = config("zero-timeout.yaml", &yaml).unwrap();
    assert_eq!(
        call(&path, ["health", "check"], "{}").await,
        ExitCode::from(2)
    );
}
