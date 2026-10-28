mod rig;

use serde_json::{Value, from_str, json};

use self::rig::{QUICK, SHORT, calls, home, logged, path, seed, start, table};
use crate::support::{SEALED, Sandbox, triage_logged};

const COOLDOWN: u64 = 3_600_000;
const DECISIONS: &str = "SELECT key || '|' || state || '|' || COALESCE(probability, '') || '|' || attempts || '|' || reason AS item FROM decisions ORDER BY key";
const HANDOFFS: &str = "SELECT key || '|' || state || '|' || attempts || '|' || COALESCE(analysis, '') || '|' || reason AS item FROM handoffs ORDER BY key";
const KEY: &str = "dingtalk:acct:m1";
const NOTHING: [&str; 0] = [];
const ONE: u32 = 1;
const SECOND: &str = "dingtalk:acct:m2";
const TARGET: &str = env!("CARGO_PKG_VERSION");
const TIMEOUT: u64 = 200;
const ZERO: u32 = 0;

#[tokio::test]
async fn codex_empty_output_counts_as_failure() {
    let sandbox = Sandbox::new("triage-codex-empty").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"codex": {"env": {"FAKE_CODEX": "empty"}}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, HANDOFFS).unwrap(),
        [format!("{KEY}|failed|2||exit:0")]
    );
    assert_eq!(calls(&sandbox, "send").unwrap(), 0);
}

#[tokio::test]
async fn codex_failure_is_retried_then_final() {
    let sandbox = Sandbox::new("triage-codex-fail").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"codex": {"env": {"FAKE_CODEX": "fail"}}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, HANDOFFS).unwrap(),
        [format!("{KEY}|failed|2||exit:3")]
    );
    assert_eq!(calls(&sandbox, "codex").unwrap(), 2);
    assert_eq!(calls(&sandbox, "send").unwrap(), 0);
}

#[tokio::test]
async fn codex_slow_is_interrupted() {
    let sandbox = Sandbox::new("triage-codex-slow").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"codex": {"env": {"FAKE_CODEX": "slow"}}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, HANDOFFS).unwrap(),
        [format!("{KEY}|pending|0||")]
    );
    assert_eq!(calls(&sandbox, "codex").unwrap(), 1);
}

#[test]
fn config_hides_variable_values() {
    let sandbox = Sandbox::new("triage-config-debug").unwrap();
    let triage = sandbox.triage(&json!({})).unwrap();
    let shown = format!("{triage:?}");
    assert!(shown.contains("Var { name: \"HOME\" }"));
}

#[test]
fn config_lists_missing_variables() {
    let sandbox = Sandbox::new("triage-config-missing").unwrap();
    assert_eq!(sandbox.triage(&json!({})).unwrap().missing(), NOTHING);
    let patch = json!({"notify_target": "${POLY_RESOLVE_UNSET_FOR_TEST}"});
    let triage = sandbox.triage(&patch).unwrap();
    assert_eq!(triage.missing(), ["POLY_RESOLVE_UNSET_FOR_TEST"]);
}

#[test]
fn config_rejects_plain_variables() {
    let sandbox = Sandbox::new("triage-config-plain").unwrap();
    for plain in ["plain", "${}", "${OPEN", "OPEN}"] {
        let patch = json!({"self_sender_id": plain});
        let err = sandbox.triage(&patch).unwrap_err();
        assert!(err.to_string().contains("expected ${NAME}"), "{plain}");
    }
}

#[tokio::test]
async fn decision_failures_are_retried_then_final() {
    let sandbox = Sandbox::new("triage-decide-fail").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"decide": {"env": {"FAKE_DECIDE": "fail"}}});
    let logs = logged(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, DECISIONS).unwrap(),
        [format!("{KEY}|failed||2|exit:22")]
    );
    assert_eq!(calls(&sandbox, "decide").unwrap(), 2);
    assert!(logs.contains("decision failed"));
}

#[tokio::test]
async fn decision_quota_limits_calls() {
    let sandbox = Sandbox::new("triage-quota").unwrap();
    seed(&sandbox, &two()).unwrap();
    let patch = json!({"limits": {"max_decisions_per_hour": ONE}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, DECISIONS).unwrap(),
        [format!("{KEY}|scored|0.95|0|")]
    );
    assert_eq!(calls(&sandbox, "decide").unwrap(), 1);
}

#[tokio::test]
async fn decision_slow_is_interrupted() {
    let sandbox = Sandbox::new("triage-decide-slow").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"decide": {"env": {"FAKE_DECIDE": "slow"}}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(table(&sandbox, DECISIONS).unwrap(), NOTHING);
    assert_eq!(calls(&sandbox, "decide").unwrap(), 1);
}

#[tokio::test]
async fn decision_unusable_answers_are_invalid() {
    let sandbox = Sandbox::new("triage-decide-junk").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"decide": {"env": {"FAKE_DECIDE": "junk"}}});
    let logs = logged(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, DECISIONS).unwrap(),
        [format!("{KEY}|invalid||0|bad_answer")]
    );
    assert_eq!(calls(&sandbox, "decide").unwrap(), 1);
    assert!(logs.contains("decision answer unusable"));
}

#[tokio::test]
async fn disabled_mode_runs_nothing() {
    let sandbox = Sandbox::new("triage-disabled").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"mode": "disabled"});
    let logs = logged(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(logs, "");
    assert_eq!(calls(&sandbox, "decide").unwrap(), 0);
    assert_eq!(table(&sandbox, DECISIONS).unwrap(), NOTHING);
}

#[tokio::test]
async fn exec_missing_program_is_reported() {
    let sandbox = Sandbox::new("triage-exec-missing").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"decide": {"bin": "/nonexistent/fake-triage"}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, DECISIONS).unwrap(),
        [format!("{KEY}|failed||2|io:NotFound")]
    );
}

#[tokio::test]
async fn exec_timeout_is_reported() {
    let sandbox = Sandbox::new("triage-exec-timeout").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"decide": {"env": {"FAKE_DECIDE": "slow"}, "timeout_ms": TIMEOUT}});
    start(&sandbox, &patch, SHORT).await.unwrap();
    assert_eq!(
        table(&sandbox, DECISIONS).unwrap(),
        [format!("{KEY}|failed||2|io:TimedOut")]
    );
}

#[tokio::test]
async fn handoff_cap_blocks_opening() {
    let sandbox = Sandbox::new("triage-cap").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"limits": {"max_handoffs_per_hour": ZERO}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, DECISIONS).unwrap(),
        [format!("{KEY}|scored|0.95|0|")]
    );
    assert_eq!(table(&sandbox, HANDOFFS).unwrap(), NOTHING);
    assert_eq!(calls(&sandbox, "codex").unwrap(), 0);
}

#[tokio::test]
async fn handoff_cooldown_merges_conversation() {
    let sandbox = Sandbox::new("triage-cooldown").unwrap();
    seed(&sandbox, &two()).unwrap();
    let patch = json!({"limits": {"handoff_cooldown_ms": COOLDOWN}});
    start(&sandbox, &patch, SHORT).await.unwrap();
    assert_eq!(table(&sandbox, DECISIONS).unwrap().len(), 2);
    assert_eq!(
        table(&sandbox, HANDOFFS).unwrap(),
        [format!("{KEY}|sent|0|ANALYSIS|")]
    );
    assert_eq!(calls(&sandbox, "codex").unwrap(), 1);
}

#[tokio::test]
async fn ledger_open_failure_is_logged() {
    let sandbox = Sandbox::new("triage-ledger-open").unwrap();
    let ingest = sandbox
        .config(&json!({"store_path": "/nonexistent/dir/store.db"}))
        .unwrap();
    let triage = sandbox.triage(&json!({})).unwrap();
    let logs = triage_logged(&ingest, &triage, QUICK).await.unwrap();
    assert!(logs.contains("triage ledger open failed"));
}

#[tokio::test]
async fn ledger_read_failure_is_logged() {
    let sandbox = Sandbox::new("triage-ledger-read").unwrap();
    seed(&sandbox, &one()).unwrap();
    sandbox
        .seed("DROP TABLE decisions; CREATE TABLE decisions (key TEXT);")
        .unwrap();
    let logs = logged(&sandbox, &json!({}), QUICK).await.unwrap();
    assert!(logs.contains("ledger read failed"));
    assert_eq!(calls(&sandbox, "decide").unwrap(), 0);
}

#[tokio::test]
async fn ledger_schema_failure_is_logged() {
    let sandbox = Sandbox::new("triage-ledger-schema").unwrap();
    sandbox.write("store.db", "not a database").unwrap();
    let logs = logged(&sandbox, &json!({}), QUICK).await.unwrap();
    assert!(logs.contains("triage ledger open failed"));
}

#[tokio::test]
async fn ledger_write_failure_is_logged() {
    let sandbox = Sandbox::new("triage-ledger-write").unwrap();
    seed(&sandbox, &one()).unwrap();
    sandbox
        .seed("CREATE TRIGGER refuse BEFORE INSERT ON decisions BEGIN SELECT RAISE(ABORT, 'refused'); END;")
        .unwrap();
    let logs = logged(&sandbox, &json!({}), QUICK).await.unwrap();
    assert!(logs.contains("ledger write failed"));
    assert_eq!(table(&sandbox, DECISIONS).unwrap(), NOTHING);
}

#[tokio::test]
async fn messages_not_for_decision_are_skipped() {
    let sandbox = Sandbox::new("triage-skips").unwrap();
    let own = home().unwrap();
    let rows = [
        (KEY, own.as_str(), "c1", "mine"),
        (SECOND, "u1", env!("CARGO_PKG_NAME"), "notice"),
        ("dingtalk:acct:m3", "u1", "c1", "  "),
        ("dingtalk:acct:m4", "u1", "c1", SEALED),
    ];
    seed(&sandbox, &rows).unwrap();
    start(&sandbox, &json!({}), QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, DECISIONS).unwrap(),
        [
            format!("{KEY}|skipped||0|own"),
            format!("{SECOND}|skipped||0|notice"),
            "dingtalk:acct:m3|skipped||0|empty".to_owned(),
            "dingtalk:acct:m4|skipped||0|sealed".to_owned(),
        ]
    );
    assert_eq!(calls(&sandbox, "decide").unwrap(), 0);
}

#[tokio::test]
async fn normal_message_is_scored_only() {
    let sandbox = Sandbox::new("triage-normal").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"decide": {"env": {"FAKE_P": "0.1"}}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, DECISIONS).unwrap(),
        [format!("{KEY}|scored|0.1|0|")]
    );
    assert_eq!(table(&sandbox, HANDOFFS).unwrap(), NOTHING);
    assert_eq!(calls(&sandbox, "codex").unwrap(), 0);
}

const fn one() -> [rig::Seed<'static>; 1] {
    [(KEY, "u1", "c1", "server down")]
}

#[tokio::test]
async fn send_failure_keeps_analysis() {
    let sandbox = Sandbox::new("triage-send-fail").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"notify": {"env": {"FAKE_SEND": "fail"}}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, HANDOFFS).unwrap(),
        [format!("{KEY}|failed|2|ANALYSIS|exit:4")]
    );
    assert_eq!(calls(&sandbox, "codex").unwrap(), 1);
    assert_eq!(calls(&sandbox, "send").unwrap(), 2);
}

#[tokio::test]
async fn send_slow_is_interrupted() {
    let sandbox = Sandbox::new("triage-send-slow").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"notify": {"env": {"FAKE_SEND": "slow"}}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, HANDOFFS).unwrap(),
        [format!("{KEY}|analyzed|0|ANALYSIS|")]
    );
    assert_eq!(calls(&sandbox, "send").unwrap(), 1);
}

const fn two() -> [rig::Seed<'static>; 2] {
    [
        (KEY, "u1", "c1", "server down"),
        (SECOND, "u1", "c1", "still down"),
    ]
}

#[tokio::test]
async fn urgent_message_is_analyzed_and_delivered() {
    let sandbox = Sandbox::new("triage-urgent").unwrap();
    seed(&sandbox, &one()).unwrap();
    let logs = logged(&sandbox, &json!({}), SHORT).await.unwrap();
    assert_eq!(
        table(&sandbox, DECISIONS).unwrap(),
        [format!("{KEY}|scored|0.95|0|")]
    );
    assert_eq!(
        table(&sandbox, HANDOFFS).unwrap(),
        [format!("{KEY}|sent|0|ANALYSIS|")]
    );
    let decided: Value = from_str(&sandbox.read("decide-stdin.txt").unwrap()).unwrap();
    assert_eq!(
        decided,
        json!({
            "input": "server down",
            "model": "m",
            "questions": [{"instructions": "is it urgent", "name": "urgent", "type": "predicate"}],
        })
    );
    let injected = format!("{}|{}\n", home().unwrap(), path().unwrap());
    assert_eq!(sandbox.read("decide-env.txt").unwrap(), injected);
    let prompt = sandbox.read("codex-stdin.txt").unwrap();
    assert!(prompt.starts_with("ctx:[") && prompt.contains("server down"));
    assert_eq!(
        sandbox.read("send-args.txt").unwrap(),
        format!("send {TARGET} --idempotency-key={KEY}\n")
    );
    assert_eq!(
        sandbox.read("send-stdin.txt").unwrap(),
        "[0.95] c1|ANALYSIS"
    );
    for line in ["urgent message", "handoff analyzed", "handoff sent"] {
        assert!(logs.contains(line), "{line}");
    }
    assert!(!logs.contains("server down"));
}
