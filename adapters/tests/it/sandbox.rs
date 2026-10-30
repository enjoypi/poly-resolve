use std::io::ErrorKind;
use std::net::TcpListener;

use serde_json::json;

use crate::support::{OPEN, Sandbox, run_for};

const SHORT: u64 = 4_000;

fn bin() -> String {
    concat!(env!("CARGO_MANIFEST_DIR"), "/tests/it/fixtures/fake-dws.sh").to_owned()
}

#[tokio::test]
async fn check_passes_for_valid_sandboxes() {
    let sandbox = Sandbox::new("sandbox-check-ok").unwrap();
    sandbox.config(&json!({})).unwrap().check().await.unwrap();
    sandbox.triage(&json!({})).unwrap().check().await.unwrap();
    let screening = sandbox.triage(&sandbox.screen("")).unwrap();
    screening.check().await.unwrap();
}

#[tokio::test]
async fn check_rejects_canary_that_is_not_denied() {
    let sandbox = Sandbox::new("sandbox-check-open").unwrap();
    let patch = json!({"dws": {"lookup": {"sandbox": {"profile": OPEN}}}});
    let err = sandbox.config(&patch).unwrap().check().await.unwrap_err();
    assert_eq!(
        err,
        format!("{}: canary /usr/bin/true was not denied", bin())
    );
}

#[tokio::test]
async fn check_rejects_failing_probe() {
    let sandbox = Sandbox::new("sandbox-check-probe").unwrap();
    for action in ["decrypt", "listen", "lookup", "pull"] {
        let patch = json!({"dws": {action: {"bin": "/nonexistent/dws"}}});
        let err = sandbox.config(&patch).unwrap().check().await.unwrap_err();
        assert_eq!(err, "/nonexistent/dws: probe failed: exit status: 71");
    }
    for role in ["codex", "notify"] {
        let patch = json!({role: {"bin": "/nonexistent/triage"}});
        let err = sandbox.triage(&patch).unwrap().check().await.unwrap_err();
        assert_eq!(err, "/nonexistent/triage: probe failed: exit status: 71");
    }
}

#[tokio::test]
async fn check_reports_sandbox_that_cannot_start() {
    let sandbox = Sandbox::new("sandbox-check-start").unwrap();
    let broken = json!({"sandbox": {"bin": "/nonexistent/sandbox-exec"}});
    for action in ["decrypt", "listen", "lookup", "pull"] {
        let patch = json!({"dws": {action: broken}});
        let err = sandbox.config(&patch).unwrap().check().await.unwrap_err();
        assert_eq!(
            err,
            format!("{}: sandbox did not start: entity not found", bin())
        );
    }
    for role in ["codex", "notify"] {
        let patch = json!({role: broken});
        let err = sandbox.triage(&patch).unwrap().check().await.unwrap_err();
        assert!(err.contains("sandbox did not start"), "{role}: {err}");
    }
}

#[tokio::test]
async fn dws_cannot_connect() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    let sandbox = Sandbox::new("sandbox-dws-net").unwrap();
    let patch = json!({"dws": {"env": {"FAKE_PULL": "net", "FAKE_PORT": port}}});
    let config = sandbox.config(&patch).unwrap();
    assert_eq!(run_for(&config, SHORT).await.unwrap(), 0);
    assert_ne!(sandbox.read("pull-args.txt").unwrap(), "");
    assert_eq!(listener.accept().unwrap_err().kind(), ErrorKind::WouldBlock);
}

#[tokio::test]
async fn dws_cannot_leave_its_directory() {
    let sandbox = Sandbox::new("sandbox-dws-escape").unwrap();
    let patch = json!({"dws": {"env": {"FAKE_PULL": "escape"}}});
    let config = sandbox.config(&patch).unwrap();
    assert_eq!(run_for(&config, SHORT).await.unwrap(), 0);
    assert_ne!(sandbox.read("pull-args.txt").unwrap(), "");
    assert_eq!(sandbox.read("leaked.txt").unwrap(), "");
    assert!(!sandbox.outside().join("escaped.txt").exists());
}

#[tokio::test]
async fn open_profile_lets_dws_connect() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    let sandbox = Sandbox::new("sandbox-open-net").unwrap();
    let patch = json!({"dws": {
        "env": {"FAKE_PULL": "net", "FAKE_PORT": port},
        "pull": {"sandbox": {"profile": OPEN}},
    }});
    let config = sandbox.config(&patch).unwrap();
    assert_eq!(run_for(&config, SHORT).await.unwrap(), 0);
    assert!(listener.accept().unwrap().1.ip().is_loopback());
}

#[tokio::test]
async fn open_profile_lets_dws_leave_its_directory() {
    let sandbox = Sandbox::new("sandbox-open-escape").unwrap();
    let patch = json!({"dws": {
        "env": {"FAKE_PULL": "escape"},
        "pull": {"sandbox": {"profile": OPEN}},
    }});
    let config = sandbox.config(&patch).unwrap();
    assert_eq!(run_for(&config, SHORT).await.unwrap(), 0);
    assert_eq!(sandbox.read("leaked.txt").unwrap(), "secret");
    assert!(sandbox.outside().join("escaped.txt").exists());
}
