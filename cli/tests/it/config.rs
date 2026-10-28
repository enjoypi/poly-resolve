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
async fn invalid_triage_variable_fails() {
    let yaml = YAML.replace("self_sender_id: \"${HOME}\"", "self_sender_id: plain");
    let path = config("invalid-triage-var.yaml", &yaml).unwrap();
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
async fn missing_ingest_fails() {
    let yaml = YAML.split("ingest:").next().unwrap_or_default();
    let path = config("missing-ingest.yaml", yaml).unwrap();
    assert_eq!(
        call(&path, ["health", "check"], "{}").await,
        ExitCode::from(2)
    );
}

#[tokio::test]
async fn missing_messages_fails() {
    let yaml = YAML.replace("messages:\n  default: 2\n  max: 3\n", "");
    let path = config("missing-messages.yaml", &yaml).unwrap();
    assert_eq!(
        call(&path, ["health", "check"], "{}").await,
        ExitCode::from(2)
    );
}

#[tokio::test]
async fn missing_web_dir_fails() {
    let yaml = YAML.replace("web_dir: /nonexistent/web\n", "");
    let path = config("missing-web-dir.yaml", &yaml).unwrap();
    assert_eq!(
        call(&path, ["health", "check"], "{}").await,
        ExitCode::from(2)
    );
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
async fn serve_refuses_enabled_triage_without_variables() {
    let yaml = YAML.replace("mode: disabled", "mode: enabled").replace(
        "notify_target: \"${HOME}\"",
        "notify_target: \"${POLY_RESOLVE_UNSET_FOR_TEST}\"",
    );
    let path = config("triage-missing-var.yaml", &yaml).unwrap();
    let code = run(&["cli", "serve", "--config", &path]).await;
    assert_eq!(code, ExitCode::from(2));
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
async fn unknown_ingest_field_fails() {
    let yaml = YAML.replace("  source: dingtalk\n", "  source: dingtalk\n  extra: 1\n");
    let path = config("unknown-ingest-field.yaml", &yaml).unwrap();
    assert_eq!(
        call(&path, ["health", "check"], "{}").await,
        ExitCode::from(2)
    );
}

#[tokio::test]
async fn zero_default_limit_fails() {
    let yaml = YAML.replace("default: 2", "default: 0");
    let path = config("zero-limit.yaml", &yaml).unwrap();
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
async fn zero_ingest_failure_exit_code_fails() {
    let yaml = YAML.replace("failure_exit_code: 16", "failure_exit_code: 0");
    let path = config("zero-ingest-exit.yaml", &yaml).unwrap();
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
