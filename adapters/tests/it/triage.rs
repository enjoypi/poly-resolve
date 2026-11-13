mod backlog;
mod batch;
mod claim;
mod clues;
mod endpoint;
mod hold;
mod lifecycle;
mod mailbox;
mod migrate;
mod notices;
mod policy;
mod prompts;
mod purge;
mod regroup;
mod remind;
mod repack;
mod revive;
mod rig;
mod screen;
mod seal;
pub(crate) mod stub;
mod thread;
mod topic;
mod usage;
mod withheld;

use serde_json::{Value, json};

use self::rig::{
    ALERTS, BATCHES, CALLS, KEY, LINES, NOTHING, PLACEMENTS, QUICK, SECOND, SHORT, TOPICS, calls,
    decided, home, input_of, logged, one, questions_of, requests, seed, start, table, text,
};
use adapters::store::Archive;

use crate::support::{BUSY, NOON, Run, SEALED, Sandbox};

const FAILED_TWICE: &str = "1|failed|2|";
const TARGET: &str = env!("CARGO_PKG_VERSION");
const TIMEOUT: u64 = 200;

fn assert_external_logs(logs: &str) {
    for field in [
        "\"feature\":\"triage\"",
        "\"action\":\"decide\"",
        "\"action\":\"analyze\"",
        "\"action\":\"notify\"",
        "\"request\":\"redacted:",
        "\"response\":\"redacted:",
        "\"elapsed_us\"",
        "\"attempt\":1",
        "\"code\":200",
    ] {
        assert!(logs.contains(field), "{field}");
    }
}

#[tokio::test]
async fn codex_cannot_leave_its_directory() {
    let sandbox = Sandbox::new("triage-codex-escape").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"codex": {"env": {"FAKE_CODEX": "escape"}}});
    start(&sandbox, &patch, SHORT).await.unwrap();
    assert_eq!(calls(&sandbox, "codex").unwrap(), 1);
    assert_eq!(sandbox.read("leaked.txt").unwrap(), "");
    assert!(!sandbox.outside().join("escaped.txt").exists());
}

#[tokio::test]
async fn codex_empty_output_counts_as_failure() {
    let sandbox = Sandbox::new("triage-codex-empty").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"codex": {"env": {"FAKE_CODEX": "empty"}}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, ALERTS).unwrap(),
        ["t1-0@1|failed|2||exit:0"]
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
        table(&sandbox, ALERTS).unwrap(),
        ["t1-0@1|failed|2||exit:3"]
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
    assert_eq!(table(&sandbox, ALERTS).unwrap(), ["t1-0@1|pending|0||"]);
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
    let unset = json!({"codex": {"sandbox": {"vars": {"HOME": "${POLY_RESOLVE_UNSET_FOR_TEST}"}}}});
    let sandboxed = sandbox.triage(&unset).unwrap();
    assert_eq!(sandboxed.missing(), ["POLY_RESOLVE_UNSET_FOR_TEST"]);
    let key = "POLY_RESOLVE_UNSET_FOR_TEST";
    let decide = json!({"decide": {"key": "${POLY_RESOLVE_UNSET_FOR_TEST}"}});
    assert_eq!(sandbox.triage(&decide).unwrap().missing(), [key]);
    let proxy = json!({"decide": {"proxy": "${POLY_RESOLVE_UNSET_FOR_TEST}"}});
    assert_eq!(sandbox.triage(&proxy).unwrap().missing(), [key]);
    let mut screen = sandbox.screen("");
    if let Some(http) = screen
        .pointer_mut("/screen/http")
        .and_then(Value::as_object_mut)
    {
        drop(http.insert("url".to_owned(), json!(format!("${{{key}}}"))));
    }
    assert_eq!(sandbox.triage(&screen).unwrap().missing(), [key]);
}

#[test]
fn config_rejects_plain_variables() {
    let sandbox = Sandbox::new("triage-config-plain").unwrap();
    for plain in ["plain", "${}", "${OPEN", "OPEN}"] {
        let patch = json!({"notify_target": plain});
        let err = sandbox.triage(&patch).unwrap_err();
        assert!(err.to_string().contains("expected ${NAME}"), "{plain}");
    }
}

#[tokio::test]
async fn decision_failures_are_retried_then_final() {
    let sandbox = Sandbox::new("triage-decide-fail").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"stub": {"decide": {"mode": "fail"}}});
    let logs = logged(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, BATCHES).unwrap(),
        [format!("{FAILED_TWICE}http:500")]
    );
    assert_eq!(table(&sandbox, PLACEMENTS).unwrap(), NOTHING);
    assert_eq!(calls(&sandbox, "decide").unwrap(), 2);
    assert!(logs.contains("decision failed"));
}

#[tokio::test]
async fn decision_slow_is_interrupted() {
    let sandbox = Sandbox::new("triage-decide-slow").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"stub": {"decide": {"mode": "slow"}}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(table(&sandbox, BATCHES).unwrap(), ["1|ready|0|"]);
    assert_eq!(table(&sandbox, CALLS).unwrap(), NOTHING);
    assert_eq!(calls(&sandbox, "decide").unwrap(), 1);
}

#[tokio::test]
async fn decision_timeout_is_reported() {
    let sandbox = Sandbox::new("triage-decide-timeout").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({
        "decide": {"timeout_ms": TIMEOUT},
        "stub": {"decide": {"mode": "slow"}},
    });
    start(&sandbox, &patch, SHORT).await.unwrap();
    assert_eq!(
        table(&sandbox, BATCHES).unwrap(),
        [format!("{FAILED_TWICE}timeout:read")]
    );
}

#[tokio::test]
async fn decision_unusable_answers_are_retried_then_final() {
    let sandbox = Sandbox::new("triage-decide-junk").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"stub": {"decide": {"mode": "junk"}}});
    let logs = logged(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, BATCHES).unwrap(),
        [format!("{FAILED_TWICE}bad_answer")]
    );
    assert!(logs.contains("decision failed"));
}

#[tokio::test]
async fn disabled_mode_runs_nothing() {
    let sandbox = Sandbox::new("triage-disabled").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"mode": "disabled"});
    let logs = logged(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(logs, "");
    assert_eq!(calls(&sandbox, "decide").unwrap(), 0);
    assert_eq!(table(&sandbox, LINES).unwrap(), NOTHING);
}

#[tokio::test]
async fn ledger_migration_failure_is_logged() {
    let sandbox = Sandbox::new("triage-ledger-migrate").unwrap();
    drop(Archive::open(&sandbox.database(), BUSY).unwrap());
    sandbox
        .seed("INSERT INTO topics (id, label, probability, thread_id, until_ms, until_key, thread_at_ms, created_at_ms, last_at_ms) VALUES ('t', 'l', NULL, 'T', 0, '', 0, 0, 0); CREATE TRIGGER refuse BEFORE INSERT ON topics BEGIN SELECT RAISE(ABORT, 'refused'); END;")
        .unwrap();
    let logs = logged(&sandbox, &json!({}), QUICK).await.unwrap();
    assert!(logs.contains("triage ledger open failed"));
}

#[tokio::test]
async fn ledger_open_failure_is_logged() {
    let sandbox = Sandbox::new("triage-ledger-open").unwrap();
    let ingest = sandbox
        .config(&json!({"store_path": "/nonexistent/dir/store.db"}))
        .unwrap();
    let triage = sandbox.triage(&json!({})).unwrap();
    let logs = sandbox
        .triage_logged(&Run::new(&ingest, &triage), QUICK)
        .await
        .unwrap();
    assert!(logs.contains("triage ledger open failed"));
}

#[tokio::test]
async fn ledger_read_failure_is_logged() {
    let sandbox = Sandbox::new("triage-ledger-read").unwrap();
    seed(&sandbox, &one()).unwrap();
    sandbox
        .seed(
            "DROP TABLE batches; CREATE TABLE batches (id INTEGER, state TEXT, revivals INTEGER);",
        )
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
async fn ledger_transaction_failure_rolls_back() {
    let sandbox = Sandbox::new("triage-ledger-tx").unwrap();
    seed(&sandbox, &one()).unwrap();
    sandbox
        .seed("CREATE TRIGGER refuse BEFORE INSERT ON alerts BEGIN SELECT RAISE(ABORT, 'refused'); END;")
        .unwrap();
    let logs = logged(&sandbox, &json!({}), QUICK).await.unwrap();
    assert!(logs.contains("ledger transaction failed"));
    assert_eq!(table(&sandbox, PLACEMENTS).unwrap(), NOTHING);
    assert_eq!(table(&sandbox, TOPICS).unwrap(), NOTHING);
}

#[tokio::test]
async fn ledger_write_failure_is_logged() {
    let sandbox = Sandbox::new("triage-ledger-write").unwrap();
    seed(&sandbox, &one()).unwrap();
    sandbox
        .seed("CREATE TRIGGER refuse BEFORE INSERT ON lines BEGIN SELECT RAISE(ABORT, 'refused'); END;")
        .unwrap();
    let logs = logged(&sandbox, &json!({}), QUICK).await.unwrap();
    assert!(logs.contains("ledger write failed"));
    assert_eq!(table(&sandbox, LINES).unwrap(), NOTHING);
}

#[tokio::test]
async fn messages_are_tagged_withheld_or_skipped() {
    let sandbox = Sandbox::new("triage-skips").unwrap();
    let own = home().unwrap();
    let rows = [
        (KEY, own.as_str(), "c1", "mine"),
        (SECOND, "u1", env!("CARGO_PKG_NAME"), "notice"),
        ("dingtalk:acct:m3", "u1", "c1", "  "),
        ("dingtalk:acct:m4", "u1", "c4", SEALED),
    ];
    seed(&sandbox, &rows).unwrap();
    start(&sandbox, &json!({}), QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, LINES).unwrap(),
        [
            format!("{KEY}|approved||1"),
            format!("{SECOND}|skipped|notice|0"),
            "dingtalk:acct:m3|withheld|empty|0".to_owned(),
            "dingtalk:acct:m4|withheld|sealed|0".to_owned(),
        ]
    );
    assert_eq!(
        table(&sandbox, "SELECT role AS item FROM lines ORDER BY key").unwrap(),
        ["own", "", "", ""]
    );
    assert_eq!(calls(&sandbox, "decide").unwrap(), 0);
    assert_eq!(table(&sandbox, CALLS).unwrap(), NOTHING);
    assert_eq!(table(&sandbox, BATCHES).unwrap(), ["1|sent|0|"]);
    assert_eq!(
        table(&sandbox, PLACEMENTS).unwrap(),
        [format!("{KEY}|t1-0|"), "dingtalk:acct:m3|t1-0|".to_owned()]
    );
    assert_eq!(table(&sandbox, ALERTS).unwrap(), NOTHING);
}

#[tokio::test]
async fn normal_message_is_scored_only() {
    let sandbox = Sandbox::new("triage-normal").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"stub": {"decide": {"p": "0.1"}}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, PLACEMENTS).unwrap(),
        [format!("{KEY}|t1-0|0.1")]
    );
    assert_eq!(table(&sandbox, ALERTS).unwrap(), NOTHING);
    assert_eq!(calls(&sandbox, "codex").unwrap(), 0);
}

#[tokio::test]
async fn other_sources_are_not_judged_by_dingtalk_identity() {
    let sandbox = Sandbox::new("triage-other-source").unwrap();
    let archive = Archive::open(&sandbox.database(), BUSY).unwrap();
    let envelope = json!({
        "account": "acct",
        "conversation_id": env!("CARGO_PKG_NAME"),
        "key": "imap:acct:m1",
        "occurred_at_ms": NOON,
        "raw": "{}",
        "received_at_ms": NOON,
        "sender_id": home().unwrap(),
        "sender_name": "",
        "source": "imap",
        "text": SEALED,
    });
    assert_eq!(archive.save_event(&envelope.to_string()).unwrap(), 1);
    let ingest = sandbox.config(&json!({})).unwrap();
    let triage = sandbox.triage(&json!({})).unwrap();
    drop(
        sandbox
            .triage_logged(&Run::new(&ingest, &triage), QUICK)
            .await
            .unwrap(),
    );
    let lines = table(&sandbox, LINES).unwrap();
    assert_eq!(lines, ["imap:acct:m1|approved||1"]);
    assert_eq!(
        table(&sandbox, "SELECT role AS item FROM lines").unwrap(),
        [""]
    );
}

#[tokio::test]
async fn send_failure_keeps_analysis() {
    let sandbox = Sandbox::new("triage-send-fail").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"notify": {"env": {"FAKE_SEND": "fail"}}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, ALERTS).unwrap(),
        ["t1-0@1|failed|2|ANALYSIS|exit:4"]
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
        table(&sandbox, ALERTS).unwrap(),
        ["t1-0@1|analyzed|0|ANALYSIS|"]
    );
    assert_eq!(calls(&sandbox, "send").unwrap(), 1);
}

#[tokio::test]
async fn unreachable_decide_endpoint_is_reported() {
    let sandbox = Sandbox::new("triage-decide-unreachable").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"decide": {"url": "http://127.0.0.1:1/decisions"}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, BATCHES).unwrap(),
        [format!("{FAILED_TWICE}connect")]
    );
    assert_eq!(calls(&sandbox, "decide").unwrap(), 0);
}

#[tokio::test]
async fn urgent_message_is_analyzed_and_delivered() {
    let sandbox = Sandbox::new("triage-urgent").unwrap();
    seed(&sandbox, &one()).unwrap();
    let logs = logged(&sandbox, &json!({}), SHORT).await.unwrap();
    assert_eq!(
        table(&sandbox, LINES).unwrap(),
        [format!("{KEY}|approved||1")]
    );
    assert_eq!(table(&sandbox, BATCHES).unwrap(), ["1|sent|0|"]);
    assert_eq!(
        table(&sandbox, PLACEMENTS).unwrap(),
        [format!("{KEY}|T1|0.95")]
    );
    assert_eq!(table(&sandbox, TOPICS).unwrap(), ["T1|server d|T1"]);
    assert_eq!(
        table(&sandbox, ALERTS).unwrap(),
        ["t1-0@1|sent|0|ANALYSIS|"]
    );
    let body = decided(&sandbox).unwrap();
    assert_eq!(
        input_of(&body).unwrap(),
        json!([{
            "conversation": "C1",
            "lines": [{
                "line": "L1",
                "sender": "name",
                "text": "server down",
                "time_utc": "2026-07-08 04:00:35",
            }],
            "segment": "S1",
        }])
    );
    assert_eq!(questions_of(&body), ["urgent_1"]);
    assert_eq!(body.get("model"), Some(&json!("m")));
    let sent = requests(&sandbox, "decide").unwrap();
    let last = sent.first().unwrap();
    assert_eq!(text(last, "method"), "POST");
    assert_eq!(text(last, "path"), "/decisions");
    assert_eq!(text(last, "content_type"), "application/json");
    assert_eq!(
        text(last, "authorization"),
        format!("Bearer {}", env!("CARGO_PKG_NAME"))
    );
    let prompt = sandbox.read("codex-stdin.txt").unwrap();
    assert!(prompt.starts_with("ctx:[") && prompt.contains("server down"));
    assert_eq!(
        sandbox.read("send-args.txt").unwrap(),
        format!("send {TARGET} --idempotency-key=t1-0@1\n")
    );
    assert_eq!(
        sandbox.read("send-stdin.txt").unwrap(),
        "[0.95] open server d T1||ANALYSIS"
    );
    for line in [
        "batch sealed",
        "batch decided",
        "alert analyzed",
        "alert sent",
    ] {
        assert!(logs.contains(line), "{line}");
    }
    assert_external_logs(&logs);
    assert!(!logs.contains("server down"));
}
