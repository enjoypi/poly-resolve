mod batch;
mod claim;
mod review;
mod rig;
mod screen;
pub(crate) mod stub;
mod thread;
mod topic;

use serde_json::{Value, json};

use self::rig::{
    ALERTS, BATCHES, CALLS, KEY, LATER, LINES, NOTHING, PLACEMENTS, QUICK, SECOND, SHORT, TOPICS,
    calls, decided, home, input_of, logged, one, questions_of, requests, seed, seed_from, start,
    table, text, two_chats,
};
use adapters::store::Archive;

use crate::support::{BUSY, NOON, Run, SEALED, Sandbox, triage_validate};

const COOLDOWN: u64 = 3_600_000;
const FAILED_TWICE: &str = "1|failed|2|";
const ONE_QUOTA: u32 = 1;
const TARGET: &str = env!("CARGO_PKG_VERSION");
const TIMEOUT: u64 = 200;
const TOO_MANY: u32 = 201;
const TOO_MANY_TOPICS: u32 = 250;
const ZERO: u32 = 0;

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
async fn decision_failures_consume_quota() {
    let sandbox = Sandbox::new("triage-quota-failure").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({
        "limits": {"max_decisions_per_hour": ONE_QUOTA},
        "stub": {"decide": {"mode": "fail"}},
    });
    start(&sandbox, &patch, QUICK).await.unwrap();
    seed_from(&sandbox, &[(SECOND, "u1", "c1", "still down")], LATER).unwrap();
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(calls(&sandbox, "decide").unwrap(), 1);
    assert_eq!(table(&sandbox, CALLS).unwrap(), ["decision|1|1"]);
    assert_eq!(table(&sandbox, BATCHES).unwrap(), ["1|ready|1|http:500"]);
}

#[tokio::test]
async fn decision_quota_limits_calls() {
    let sandbox = Sandbox::new("triage-quota").unwrap();
    seed(&sandbox, &two_chats()).unwrap();
    let patch = json!({"limits": {"max_decisions_per_hour": ZERO}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(table(&sandbox, BATCHES).unwrap(), ["1|ready|0|"]);
    assert_eq!(calls(&sandbox, "decide").unwrap(), 0);
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
        [format!("{FAILED_TWICE}timeout")]
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
async fn handoff_cap_blocks_opening() {
    let sandbox = Sandbox::new("triage-cap").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"limits": {"max_handoffs_per_hour": ZERO}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, PLACEMENTS).unwrap(),
        [format!("{KEY}|t1-0|0.95")]
    );
    assert_eq!(table(&sandbox, ALERTS).unwrap(), ["t1-0@1|pending|0||"]);
    assert_eq!(calls(&sandbox, "codex").unwrap(), 0);
}

#[tokio::test]
async fn handoff_cooldown_merges_the_topic() {
    let sandbox = Sandbox::new("triage-cooldown").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"limits": {"handoff_cooldown_ms": COOLDOWN}});
    start(&sandbox, &patch, SHORT).await.unwrap();
    seed_from(&sandbox, &[(SECOND, "u1", "c1", "still down")], LATER).unwrap();
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, PLACEMENTS).unwrap(),
        [format!("{KEY}|T1|0.95"), format!("{SECOND}|T1|0.95"),]
    );
    assert_eq!(
        table(&sandbox, ALERTS).unwrap(),
        ["T1@2|pending|0||", "t1-0@1|sent|0|ANALYSIS|"]
    );
    assert_eq!(calls(&sandbox, "decide").unwrap(), 2);
    assert_eq!(calls(&sandbox, "codex").unwrap(), 1);
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
        .seed("DROP TABLE batches; CREATE TABLE batches (id INTEGER);")
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
async fn messages_not_for_decision_are_skipped() {
    let sandbox = Sandbox::new("triage-skips").unwrap();
    let own = home().unwrap();
    let rows = [
        (KEY, own.as_str(), "c1", "mine"),
        (SECOND, "u1", env!("CARGO_PKG_NAME"), "notice"),
        ("dingtalk:acct:m3", "u1", "c3", "  "),
        ("dingtalk:acct:m4", "u1", "c4", SEALED),
    ];
    seed(&sandbox, &rows).unwrap();
    start(&sandbox, &json!({}), QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, LINES).unwrap(),
        [
            format!("{KEY}|skipped|own|0"),
            format!("{SECOND}|skipped|notice|0"),
            "dingtalk:acct:m3|skipped|empty|0".to_owned(),
            "dingtalk:acct:m4|skipped|sealed|0".to_owned(),
        ]
    );
    assert_eq!(calls(&sandbox, "decide").unwrap(), 0);
    assert_eq!(table(&sandbox, BATCHES).unwrap(), NOTHING);
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
    assert_eq!(lines.len(), 1);
    assert!(!lines.concat().contains("skipped"), "{lines:?}");
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
        "[0.95] T1|ANALYSIS"
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

#[test]
fn validate_checks_limits_and_rules() {
    let sandbox = Sandbox::new("triage-validate").unwrap();
    let ingest = sandbox.config(&json!({})).unwrap();
    let checked = |patch: &Value| {
        let triage = sandbox.triage(patch).unwrap();
        match triage_validate(&ingest, &triage) {
            Ok(result) => result.map_err(|err| format!("{patch}: {err}")),
            Err(err) => Err(err.to_string()),
        }
    };
    assert_eq!(checked(&json!({})), Ok(()));
    let batch = json!({"limits": {"batch": TOO_MANY}});
    assert!(checked(&batch).unwrap_err().contains("too large"));
    let topics = json!({"sizes": {"topics_max_open": TOO_MANY_TOPICS}});
    assert!(checked(&topics).unwrap_err().contains("too large"));
    let external = json!({"decide": {"url": "https://localhost/decisions"}});
    assert!(checked(&external).unwrap_err().contains("url invalid"));
    let invalid = json!({"decide": {"url": "not a url"}});
    assert!(checked(&invalid).unwrap_err().contains("url invalid"));
    let credentials = json!({"decide": {"url": "https://user@example.com/decisions"}});
    assert!(checked(&credentials).unwrap_err().contains("url invalid"));
    let remote = json!({"screen": {"http": {"url": "https://example.com/model"}}});
    assert!(checked(&remote).unwrap_err().contains("url invalid"));
    let no_port = json!({"screen": {"http": {"url": "http://localhost/model"}}});
    assert!(checked(&no_port).unwrap_err().contains("url invalid"));
    assert_eq!(checked(&sandbox.screen("")), Ok(()));
    let rules = json!({"redact": {"patterns": [{"pattern": "(", "placeholder": "x"}]}});
    assert!(checked(&rules).unwrap_err().contains("invalid pattern"));
}
