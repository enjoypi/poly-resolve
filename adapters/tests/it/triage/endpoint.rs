use std::net::TcpListener;

use serde_json::{Value, json};

use super::rig::{BATCHES, NOTHING, QUICK, calls, logged, one, seed, start, table, two};
use crate::support::{Sandbox, triage_reach, triage_validate};

const BRIEF: u64 = 1_000;
const TOO_MANY: u32 = 201;
const TOO_MANY_TOPICS: u32 = 250;

fn closed() -> String {
    let port = TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .map_or_else(|_err| u16::default(), |address| address.port());
    format!("http://127.0.0.1:{port}/v1")
}

#[tokio::test]
async fn invalid_decision_url_keeps_the_batch_unsent() {
    let sandbox = Sandbox::new("endpoint-decide").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"decide": {"url": "https://localhost/decisions"}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(calls(&sandbox, "decide").unwrap(), 0);
    assert_eq!(
        table(&sandbox, BATCHES).unwrap(),
        ["1|ready|0|external url must use ${NAME}"]
    );
    let proxied = Sandbox::new("endpoint-decide-proxy").unwrap();
    seed(&proxied, &one()).unwrap();
    let bad = json!({"decide": {"proxy": "${HOME}"}});
    start(&proxied, &bad, QUICK).await.unwrap();
    assert_eq!(calls(&proxied, "decide").unwrap(), 0);
    assert_eq!(
        table(&proxied, BATCHES).unwrap(),
        ["1|ready|0|proxy must be http://<loopback>:<port>"]
    );
}

#[tokio::test]
async fn invalid_screen_url_defers_the_lines() {
    let sandbox = Sandbox::new("endpoint-screen").unwrap();
    seed(&sandbox, &two()).unwrap();
    let logs = logged(&sandbox, &located(&sandbox, "not a url"), QUICK)
        .await
        .unwrap();
    assert!(logs.contains("screening failed"));
    assert_eq!(calls(&sandbox, "screen").unwrap(), 0);
    assert_eq!(calls(&sandbox, "decide").unwrap(), 0);
    assert_eq!(table(&sandbox, BATCHES).unwrap(), NOTHING);
}

fn located(sandbox: &Sandbox, url: &str) -> Value {
    let mut patch = sandbox.screen("");
    if let Some(slot) = patch.pointer_mut("/screen/http/url") {
        *slot = json!(url);
    }
    patch
}

#[tokio::test]
async fn reach_checks_decision_and_codex_endpoints() {
    let sandbox = Sandbox::new("endpoint-reach").unwrap();
    let local = sandbox
        .screen("")
        .pointer("/screen/http")
        .cloned()
        .unwrap_or_default();
    let checked = async |patch: &Value| triage_reach(&sandbox.triage(patch).unwrap()).await;
    assert_eq!(checked(&json!({})).await, Ok(()));
    let codex = json!({"codex": {"endpoint": local}});
    assert_eq!(checked(&codex).await, Ok(()));
    let down = json!({"timeout_ms": BRIEF, "url": closed()});
    let decide = json!({"decide": down});
    assert_eq!(
        checked(&decide).await,
        Err("triage.decide: unreachable connect".to_owned())
    );
    let invalid = json!({"decide": {"timeout_ms": BRIEF, "url": "not a url"}});
    assert!(checked(&invalid).await.unwrap_err().contains("invalid url"));
    let codex_down = json!({"codex": {"endpoint": down}});
    assert_eq!(
        checked(&codex_down).await,
        Err("triage.codex.endpoint: unreachable connect".to_owned())
    );
    let unset = json!({"codex": {"endpoint": {"timeout_ms": BRIEF, "url": "${POLY_RESOLVE_UNSET_FOR_TEST}"}}});
    assert_eq!(
        sandbox.triage(&unset).unwrap().missing(),
        ["POLY_RESOLVE_UNSET_FOR_TEST"]
    );
}

#[test]
fn validate_checks_limits_and_rules() {
    let sandbox = Sandbox::new("triage-validate").unwrap();
    let checked = |patch: &Value| validated(&sandbox, patch);
    assert_eq!(checked(&json!({})), Ok(()));
    let batch = json!({"limits": {"batch": TOO_MANY}});
    assert!(checked(&batch).unwrap_err().contains("too large"));
    let topics = json!({"sizes": {"topics_max_open": TOO_MANY_TOPICS}});
    assert!(checked(&topics).unwrap_err().contains("too large"));
    for url in [
        "https://localhost/decisions",
        "not a url",
        "https://user@example.com/decisions",
        "/decisions",
        "localhost:8080",
        "ftp://localhost/decisions",
        "http://example.com/decisions",
    ] {
        let patch = json!({"decide": {"url": url}});
        assert!(
            checked(&patch).unwrap_err().contains("triage.decide:"),
            "{url}"
        );
    }
    for url in [
        "https://example.com/model",
        "http://localhost/model",
        "http://example.com:80/model",
    ] {
        let patch = located(&sandbox, url);
        assert!(
            checked(&patch).unwrap_err().contains("triage.screen.http:"),
            "{url}"
        );
    }
    assert_eq!(checked(&sandbox.screen("")), Ok(()));
    let rules = json!({"redact": {"patterns": [{"pattern": "(", "placeholder": "x"}]}});
    assert!(checked(&rules).unwrap_err().contains("invalid pattern"));
}

#[test]
fn validate_checks_proxies() {
    let sandbox = Sandbox::new("triage-validate-proxy").unwrap();
    let checked = |patch: &Value| validated(&sandbox, patch);
    for (proxy, reason) in [
        ("${POLY_RESOLVE_TEST_PROXY}", "proxy requires https url"),
        ("${POLY_RESOLVE_TEST_BAD_PROXY}", "proxy must be"),
        ("${HOME}", "proxy must be"),
        ("${POLY_RESOLVE_UNSET_FOR_TEST}", "proxy must be"),
    ] {
        let patch = json!({"decide": {"proxy": proxy}});
        assert!(checked(&patch).unwrap_err().contains(reason), "{proxy}");
    }
    let https = json!({"decide": {
        "proxy": "${POLY_RESOLVE_TEST_PROXY}",
        "url": "https://localhost/decisions",
    }});
    assert!(checked(&https).unwrap_err().contains("must use"));
    let mut proxied = sandbox.screen("");
    if let Some(http) = proxied
        .pointer_mut("/screen/http")
        .and_then(Value::as_object_mut)
    {
        drop(http.insert("proxy".to_owned(), json!("${POLY_RESOLVE_TEST_PROXY}")));
    }
    assert!(
        checked(&proxied)
            .unwrap_err()
            .contains("must not use proxy")
    );
}

fn validated(sandbox: &Sandbox, patch: &Value) -> Result<(), String> {
    let ingest = sandbox.config(&json!({})).map_err(|err| err.to_string())?;
    let triage = sandbox.triage(patch).map_err(|err| err.to_string())?;
    triage_validate(&ingest, &triage)
        .map_err(|err| err.to_string())?
        .map_err(|err| format!("{patch}: {err}"))
}
