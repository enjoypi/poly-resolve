use core::error::Error;
use core::time::Duration;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use adapters::config::Settings;
use adapters::ingest::{Site, run};
use adapters::triage::config::Settings as Triage;
use adapters::triage::{Lane, Setup, run as run_triage, validate};
use domain::Errors;
use domain::features::{Context, Ports, Rules};
use domain::inbox::Paging;
use futures_util::future::join;
use rusqlite::Connection;
use serde_json::{Map, Value, from_str, from_value, json};
use tokio::time::sleep;
use tracing::instrument::WithSubscriber as _;

use crate::capture::Capture;

const ATTEMPTS: u32 = 2;
const BACKFILL: u64 = 3_600_000;
const BACKOFF_INITIAL: u64 = 5;
const BACKOFF_MAX: u64 = 20;
pub(crate) const BAD_SCHEMA: &str = "CREATE TABLE messages (key TEXT PRIMARY KEY, source TEXT, account TEXT, occurred_at_ms INTEGER);";
pub(crate) const BAD_STAMP: &str = "CREATE TABLE messages (key TEXT PRIMARY KEY, source TEXT, account TEXT, conversation_id TEXT, sender_id TEXT, sender_name TEXT, text TEXT, occurred_at_ms TEXT, received_at_ms INTEGER, raw TEXT); INSERT INTO messages VALUES ('k', 'dingtalk', 'acct', 'c', 'u', 'n', 't', 'abc', 1, '{}'); CREATE TABLE placements (key TEXT, probability REAL, topic_id TEXT); CREATE TABLE alerts (topic_id TEXT, analysis TEXT, created_at_ms INTEGER);";
const BATCH: u32 = 10;
pub(crate) const BIN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/it/fixtures/fake-dws.sh");
pub(crate) const BUSY: u64 = 5_000;
const CAP_CHARS: u32 = 1_000;
const CHILD_EXIT: u64 = 300;
const CIPHER_MIN: u32 = 32;
const CONTEXT_CHARS: u32 = 10_000;
const DWS_TIMEOUT: u64 = 60_000;
const ERRORS: &str = r#"{"bad_request":{"code":"invalid"},"conflict":{"code":"clash"},"not_found":{"code":"absent"},"storage":{"code":"unavailable"},"unknown_action":{"code":"missing"},"unprocessable":{"code":"refused"}}"#;
const EXEC_TIMEOUT: u64 = 5_000;
pub(crate) const FAILURE: u8 = 7;
const FAKE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/it/fixtures/fake-triage.sh"
);
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/it/fixtures");
pub(crate) const FRAGMENTS: &str = "title\nalice:QUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVo0MTIzNDU2\nQUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVo0MTIzNDU2\nQUJDREVGR0hJSktMTU5PUFFSU1RV";
const GAP: u64 = 3_600_000;
const HEAL_LIMIT: u32 = 2;
const INTAKE: u32 = 50;
const LABEL: u32 = 8;
pub(crate) const LEARN_MIN: u32 = 4;
const LOOKBACK: u64 = 1_000_000_000_000;
const LOOKUP_BATCH: u32 = 2;
pub(crate) const MANUAL: u8 = 6;
const MANUAL_CODES: [i32; 2] = [2, 4];
const MIN_CONFIDENCE: f64 = 0.6;
pub(crate) const NOON: u64 = 1_783_483_235_000;
const OFFSET: i16 = 480;
pub(crate) const OPEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/it/fixtures/open.sb");
const OVERLAP: u64 = 1_000;
const PAGING: &str = r#"{"default":2,"max":3}"#;
const PARALLEL: u32 = 1;
const POLL: u64 = 20;
const PROFILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/it/fixtures/fake.sb");
const QUOTA: u32 = 100;
pub(crate) const REFUSE_INSERT: &str =
    "CREATE TRIGGER refuse BEFORE INSERT ON messages BEGIN SELECT RAISE(ABORT, 'refused'); END;";
pub(crate) const REFUSE_UPDATE: &str =
    "CREATE TRIGGER refuse BEFORE UPDATE ON messages BEGIN SELECT RAISE(ABORT, 'refused'); END;";
pub(crate) const SEALED: &str = "QUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVo0MTIzNDU2||2||1||196";
const SHARED: [&str; 6] = [
    "backoff_initial_ms",
    "backoff_max_ms",
    "busy_timeout_ms",
    "failure_exit_code",
    "manual_exit_code",
    "store_path",
];
pub(crate) const TEXT_STAMP: &str = "CREATE TABLE messages (key TEXT PRIMARY KEY, source TEXT, account TEXT, occurred_at_ms TEXT); INSERT INTO messages VALUES ('k', 'dingtalk', 'acct', 'abc');";
const THRESHOLD: f64 = 0.8;
const TOPICS: u32 = 5;
const ZERO: u64 = 0;

pub(crate) type Fallible<T> = Result<T, Box<dyn Error>>;

pub(crate) struct Sandbox {
    dir: PathBuf,
    outside: PathBuf,
}

impl Sandbox {
    pub(crate) fn config(&self, patch: &Value) -> Fallible<Settings> {
        let mut shared = Map::new();
        let mut own = Map::new();
        for (key, value) in patch.as_object().into_iter().flatten() {
            let target = if SHARED.contains(&key.as_str()) {
                &mut shared
            } else {
                &mut own
            };
            drop(target.insert(key.clone(), value.clone()));
        }
        let sources = json!([{"dingtalk": self.source(&Value::Object(own))}]);
        let mut base = self.shared(&sources);
        merge(&mut base, &Value::Object(shared));
        Ok(from_value(base)?)
    }

    pub(crate) fn database(&self) -> PathBuf {
        self.dir.join("store.db")
    }

    fn exec(&self, role: &str, mode: &str) -> Value {
        json!({
            "args": [role],
            "bin": FAKE,
            "env": {
                "FAKE_DIR": self.dir.to_string_lossy(),
                "FAKE_SCREEN": mode,
                "FAKE_OUTSIDE": self.outside.to_string_lossy(),
            },
            "sandbox": self.fence(),
            "timeout_ms": EXEC_TIMEOUT,
            "vars": {"PATH": "${PATH}", "TRIAGE_KEY": "${PATH}", "TRIAGE_URL": "${HOME}"},
        })
    }

    fn fence(&self) -> Value {
        json!({
            "bin": "/usr/bin/sandbox-exec",
            "canary": "/usr/bin/true",
            "paths": {"DIR": self.dir.to_string_lossy(), "FIXTURES": FIXTURES},
            "probe": ["noop"],
            "profile": PROFILE,
            "vars": {"HOME": "${HOME}"},
        })
    }

    pub(crate) fn new(name: &str) -> io::Result<Self> {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"));
        let dir = root.join(name);
        let outside = root.join(format!("{name}-outside"));
        for path in [&dir, &outside] {
            if path.exists() {
                fs::remove_dir_all(path)?;
            }
            fs::create_dir_all(path)?;
        }
        fs::write(outside.join("secret.txt"), "secret")?;
        Ok(Self { dir, outside })
    }

    pub(crate) fn outside(&self) -> &Path {
        &self.outside
    }

    pub(crate) fn read(&self, file: &str) -> io::Result<String> {
        fs::read_to_string(self.dir.join(file))
    }

    pub(crate) fn screen(&self, mode: &str) -> Value {
        json!({
            "screen": {
                "exec": self.exec("screen", mode),
                "instructions": "judge",
                "model": "local",
                "placeholder": "[s]",
            },
        })
    }

    pub(crate) fn seed(&self, sql: &str) -> Fallible<()> {
        Ok(Connection::open(self.database())?.execute_batch(sql)?)
    }

    pub(crate) fn shared(&self, sources: &Value) -> Value {
        json!({
            "backoff_initial_ms": BACKOFF_INITIAL,
            "backoff_max_ms": BACKOFF_MAX,
            "busy_timeout_ms": BUSY,
            "failure_exit_code": FAILURE,
            "manual_exit_code": MANUAL,
            "sources": sources,
            "store_path": self.database(),
        })
    }

    pub(crate) fn source(&self, patch: &Value) -> Value {
        let process = |action: &str| json!({"args": [action], "bin": BIN, "sandbox": self.fence()});
        let mut source = json!({
            "account": "acct",
            "child_exit_timeout_ms": CHILD_EXIT,
            "cipher_min_length": CIPHER_MIN,
            "dws": {
                "decrypt": process("decrypt"),
                "env": {
                    "FAKE_DIR": self.dir.to_string_lossy(),
                    "FAKE_OUTSIDE": self.outside.to_string_lossy(),
                },
                "listen": process("listen"),
                "lookup": process("lookup"),
                "manual_exit_codes": MANUAL_CODES,
                "parallel": PARALLEL,
                "pull": process("pull"),
                "timeout_ms": DWS_TIMEOUT,
                "vars": {"HOME": "${HOME}", "PATH": "${PATH}"},
            },
            "heal_limit": HEAL_LIMIT,
            "initial_backfill_ms": BACKFILL,
            "lookup_batch": LOOKUP_BATCH,
            "notify_conversation_id": "${CARGO_PKG_NAME}",
            "reconcile_overlap_ms": OVERLAP,
            "self_sender_id": "${HOME}",
            "utc_offset_minutes": OFFSET,
        });
        merge(&mut source, patch);
        source
    }

    pub(crate) fn triage(&self, patch: &Value) -> Fallible<Triage> {
        let exec = |role: &str| self.exec(role, "");
        let mut base = json!({
            "codex": exec("codex"),
            "decide": exec("decide"),
            "limits": {
                "batch": BATCH,
                "handoff_cooldown_ms": ZERO,
                "intake": INTAKE,
                "lookback_ms": LOOKBACK,
                "max_attempts": ATTEMPTS,
                "max_decisions_per_hour": QUOTA,
                "max_handoffs_per_hour": QUOTA,
                "poll_ms": POLL,
                "retry_after_ms": ZERO,
            },
            "mode": "enabled",
            "model": "m",
            "notice": "[{probability}] {topic}|{analysis}",
            "notify": exec("send"),
            "notify_target": "${CARGO_PKG_VERSION}",
            "questions": {
                "threshold": THRESHOLD,
                "topic": {
                    "instructions": "which {segment}",
                    "min_confidence": MIN_CONFIDENCE,
                    "name": "topic",
                    "new_label": "new topic",
                },
                "urgent": {"instructions": "urgent {line}", "name": "urgent"},
            },
            "review": {
                "learn_min_chars": LEARN_MIN,
                "mode": "auto",
                "reasons": [
                    {"code": "leak", "label": "Leaked", "placeholder": "[x]"},
                    {"code": "noise", "label": "Noise"},
                ],
            },
            "redact": {
                "patterns": [{"pattern": "[0-9]{11,}", "placeholder": "[num]"}],
                "unknown_sender": "unknown",
            },
        });
        merge(&mut base, &text_settings());
        merge(&mut base, patch);
        Ok(from_value(base)?)
    }

    pub(crate) fn write(&self, file: &str, body: &str) -> io::Result<()> {
        fs::write(self.dir.join(file), body)
    }
}

pub(crate) fn envelope(key: &str, occurred_at_ms: u64) -> String {
    json!({
        "account": "acct",
        "conversation_id": "c1",
        "key": key,
        "occurred_at_ms": occurred_at_ms,
        "raw": "{}",
        "received_at_ms": occurred_at_ms,
        "sender_id": "u1",
        "sender_name": "alice",
        "source": "dingtalk",
        "text": "hello",
    })
    .to_string()
}

pub(crate) fn keys(database: &Path) -> Fallible<Vec<String>> {
    let connection = Connection::open(database)?;
    let mut statement = connection.prepare("SELECT key FROM messages ORDER BY key")?;
    let mut rows = statement.query([])?;
    let mut keys = Vec::new();
    while let Some(row) = rows.next()? {
        keys.push(row.get(0)?);
    }
    Ok(keys)
}

fn merge(base: &mut Value, patch: &Value) {
    if let (Some(target), Some(source)) = (base.as_object_mut(), patch.as_object()) {
        for (key, value) in source {
            match target.get_mut(key) {
                Some(slot) => merge(slot, value),
                None => drop(target.insert(key.clone(), value.clone())),
            }
        }
    } else {
        *base = patch.clone();
    }
}

pub(crate) async fn run_for(config: &Settings, millis: u64) -> Fallible<u8> {
    Ok(run_logged(config, millis).await?.0)
}

pub(crate) async fn run_logged(config: &Settings, millis: u64) -> Fallible<(u8, String)> {
    let capture = Capture::default();
    let errors: Errors = from_str(ERRORS)?;
    let paging: Paging = from_str(PAGING)?;
    let shelf = config.shelf();
    let context = Context::new(
        &errors,
        Rules::new(&paging, &[], LEARN_MIN),
        Ports::new(&shelf, &shelf),
    );
    let mut stop = Box::pin(sleep(Duration::from_millis(millis)));
    let source = config.sources().first().ok_or("no source")?;
    let code = run(Site::new(config, &context), source, &mut stop)
        .with_subscriber(capture.dispatch())
        .await;
    Ok((code, capture.text()))
}

pub(crate) fn sealed_envelope(key: &str, occurred_at_ms: u64) -> String {
    stored(key, occurred_at_ms, SEALED)
}

pub(crate) fn stored(key: &str, occurred_at_ms: u64, text: &str) -> String {
    json!({
        "account": "acct",
        "conversation_id": "c1",
        "key": key,
        "occurred_at_ms": occurred_at_ms,
        "raw": "{}",
        "received_at_ms": occurred_at_ms,
        "sender_id": "u1",
        "sender_name": "alice",
        "source": "dingtalk",
        "text": text,
    })
    .to_string()
}

fn text_settings() -> Value {
    json!({
        "session": {
            "followup": "next:{messages}",
            "fresh": ["-"],
            "prompt": "ctx:{messages}",
            "resume": ["resume", "{thread}", "-"],
        },
        "sizes": {
            "context_max_chars": CONTEXT_CHARS,
            "label_chars": LABEL,
            "result_max_chars": CAP_CHARS,
            "session_gap_ms": GAP,
            "session_max_ms": LOOKBACK,
            "topics_max_open": TOPICS,
        },
    })
}

pub(crate) fn texts(database: &Path) -> Fallible<Vec<String>> {
    let connection = Connection::open(database)?;
    let mut statement = connection.prepare("SELECT text FROM messages ORDER BY key")?;
    let mut rows = statement.query([])?;
    let mut texts = Vec::new();
    while let Some(row) = rows.next()? {
        texts.push(row.get(0)?);
    }
    Ok(texts)
}

pub(crate) async fn triage_logged(
    ingest: &Settings,
    triage: &Triage,
    millis: u64,
) -> Fallible<String> {
    let capture = Capture::default();
    let errors: Errors = from_str(ERRORS)?;
    let paging: Paging = from_str(PAGING)?;
    let shelf = ingest.shelf();
    let context = Context::new(
        &errors,
        Rules::new(&paging, &[], LEARN_MIN),
        Ports::new(&shelf, &shelf),
    );
    let setup = Setup::new(ingest, triage, &context);
    let mut batches = Box::pin(sleep(Duration::from_millis(millis)));
    let mut alerts = Box::pin(sleep(Duration::from_millis(millis)));
    let ((), ()) = join(
        run_triage(setup, Lane::Batches, &mut batches),
        run_triage(setup, Lane::Alerts, &mut alerts),
    )
    .with_subscriber(capture.dispatch())
    .await;
    Ok(capture.text())
}

pub(crate) fn triage_validate(ingest: &Settings, triage: &Triage) -> Fallible<Result<(), String>> {
    let errors: Errors = from_str(ERRORS)?;
    let paging: Paging = from_str(PAGING)?;
    let shelf = ingest.shelf();
    let context = Context::new(
        &errors,
        Rules::new(&paging, &[], LEARN_MIN),
        Ports::new(&shelf, &shelf),
    );
    Ok(validate(Setup::new(ingest, triage, &context)))
}
