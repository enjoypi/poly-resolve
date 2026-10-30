use adapters::config::Settings;
use adapters::ingest::Source;
use serde_json::{Value, from_value, json};

use crate::support::{Sandbox, run_for};

const BRIEF: u64 = 100;

#[test]
fn identity_vars_are_reported_when_missing() {
    let sandbox = Sandbox::new("sources-missing").unwrap();
    let present = sandbox.config(&json!({})).unwrap();
    assert_eq!(present.missing(), Vec::<&str>::new());
    let absent = sandbox
        .config(&json!({"self_sender_id": "${POLY_RESOLVE_ABSENT}"}))
        .unwrap();
    assert_eq!(absent.missing(), ["POLY_RESOLVE_ABSENT"]);
}

#[tokio::test]
async fn source_without_kind_collects_nothing() {
    let sandbox = Sandbox::new("sources-bare").unwrap();
    let bare = with_sources(&sandbox, &json!([{}])).unwrap();
    assert_eq!(
        bare.sources().first().map(Source::label),
        Some(String::new())
    );
    assert_eq!(run_for(&bare, BRIEF).await.unwrap(), 0);
}

#[test]
fn sources_must_be_known_unique_and_present() {
    let sandbox = Sandbox::new("sources-verify").unwrap();
    let single = sandbox.config(&json!({})).unwrap();
    assert_eq!(single.verify(), Ok(()));
    let labels: Vec<String> = single.sources().iter().map(Source::label).collect();
    assert_eq!(labels, ["dingtalk:acct"]);
    let empty = with_sources(&sandbox, &json!([])).unwrap();
    assert_eq!(empty.verify().unwrap_err(), "ingest.sources: empty");
    let twin = json!({"dingtalk": sandbox.source(&json!({}))});
    let twins = with_sources(&sandbox, &json!([twin, twin])).unwrap();
    assert_eq!(
        twins.verify().unwrap_err(),
        "ingest.sources: duplicate source"
    );
    let unknown = with_sources(&sandbox, &json!([{"rss": {}}])).unwrap_err();
    assert!(unknown.contains("unknown field"), "{unknown}");
    let bare = with_sources(&sandbox, &json!([{}])).unwrap();
    assert_eq!(
        bare.verify().unwrap_err(),
        "ingest.sources: source without kind"
    );
    let extra = json!([{"dingtalk": sandbox.source(&json!({"extra": true}))}]);
    let rejected = with_sources(&sandbox, &extra).unwrap_err();
    assert!(rejected.contains("unknown field"), "{rejected}");
}

fn with_sources(sandbox: &Sandbox, sources: &Value) -> Result<Settings, String> {
    from_value(sandbox.shared(sources)).map_err(|err| err.to_string())
}
