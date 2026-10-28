extern crate alloc;

use alloc::sync::Arc;
use core::error::Error;
use core::time::Duration;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, PoisonError};

use adapters::config::Settings;
use adapters::ingest::run;
use domain::Errors;
use domain::features::Context;
use domain::inbox::Paging;
use rusqlite::Connection;
use serde_json::{Value, from_str, from_value, json};
use tokio::time::sleep;
use tracing::instrument::WithSubscriber as _;
use tracing::subscriber::NoSubscriber;
use tracing::{Dispatch, Level};
use tracing_subscriber::fmt;
use tracing_subscriber::fmt::MakeWriter;

static ANCHOR: LazyLock<Dispatch> = LazyLock::new(|| Dispatch::new(NoSubscriber::default()));
const BACKFILL: u64 = 3_600_000;
const BACKOFF_INITIAL: u64 = 5;
const BACKOFF_MAX: u64 = 20;
pub(crate) const BAD_SCHEMA: &str = "CREATE TABLE messages (key TEXT PRIMARY KEY, source TEXT, account TEXT, occurred_at_ms INTEGER);";
pub(crate) const BAD_STAMP: &str = "CREATE TABLE messages (key TEXT PRIMARY KEY, source TEXT, account TEXT, conversation_id TEXT, sender_id TEXT, sender_name TEXT, text TEXT, occurred_at_ms TEXT, received_at_ms INTEGER, raw TEXT); INSERT INTO messages VALUES ('k', 'dingtalk', 'acct', 'c', 'u', 'n', 't', 'abc', 1, '{}');";
pub(crate) const BIN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/it/fixtures/fake-dws.sh");
const CHILD_EXIT: u64 = 300;
const CIPHER_MIN: u32 = 32;
const ERRORS: &str = r#"{"bad_request":{"code":"invalid","exit_code":13},"storage":{"code":"unavailable","exit_code":15},"unknown_action":{"code":"missing","exit_code":14}}"#;
pub(crate) const FAILURE: u8 = 7;
pub(crate) const FRAGMENTS: &str = "title\nalice:QUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVo0MTIzNDU2\nQUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVo0MTIzNDU2\nQUJDREVGR0hJSktMTU5PUFFSU1RV";
const LOOKUP_BATCH: u32 = 2;
pub(crate) const MANUAL: u8 = 6;
const MANUAL_CODES: [i32; 2] = [2, 4];
pub(crate) const NOON: u64 = 1_783_483_235_000;
const OFFSET: i16 = 480;
const OVERLAP: u64 = 1_000;
const PAGING: &str = r#"{"default":2,"max":3}"#;
pub(crate) const SEALED: &str = "QUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVo0MTIzNDU2||2||1||196";
pub(crate) const TEXT_STAMP: &str = "CREATE TABLE messages (key TEXT PRIMARY KEY, source TEXT, account TEXT, occurred_at_ms TEXT); INSERT INTO messages VALUES ('k', 'dingtalk', 'acct', 'abc');";

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl Capture {
    fn text(&self) -> String {
        let bytes = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        String::from_utf8(bytes).unwrap_or_default()
    }
}

impl<'writer> MakeWriter<'writer> for Capture {
    type Writer = Self;

    fn make_writer(&'writer self) -> Self::Writer {
        self.clone()
    }
}

impl io::Write for Capture {
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend_from_slice(buf);
        Ok(buf.len())
    }
}

pub(crate) type Fallible<T> = Result<T, Box<dyn Error>>;

pub(crate) struct Sandbox {
    dir: PathBuf,
}

impl Sandbox {
    pub(crate) fn config(&self, patch: &Value) -> Fallible<Settings> {
        let mut base = json!({
            "account": "acct",
            "backoff_initial_ms": BACKOFF_INITIAL,
            "backoff_max_ms": BACKOFF_MAX,
            "child_exit_timeout_ms": CHILD_EXIT,
            "cipher_min_length": CIPHER_MIN,
            "dws": {
                "decrypt": {"args": ["decrypt"], "bin": BIN},
                "env": {"FAKE_DIR": self.dir.to_string_lossy()},
                "listen": {"args": ["listen"], "bin": BIN},
                "lookup": {"args": ["lookup"], "bin": BIN},
                "manual_exit_codes": MANUAL_CODES,
                "pull": {"args": ["pull"], "bin": BIN},
            },
            "failure_exit_code": FAILURE,
            "initial_backfill_ms": BACKFILL,
            "lookup_batch": LOOKUP_BATCH,
            "manual_exit_code": MANUAL,
            "reconcile_overlap_ms": OVERLAP,
            "source": "dingtalk",
            "source_utc_offset_minutes": OFFSET,
            "store_path": self.database(),
        });
        merge(&mut base, patch);
        Ok(from_value(base)?)
    }

    pub(crate) fn database(&self) -> PathBuf {
        self.dir.join("store.db")
    }

    pub(crate) fn new(name: &str) -> io::Result<Self> {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
        if dir.exists() {
            fs::remove_dir_all(&dir)?;
        }
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    pub(crate) fn read(&self, file: &str) -> io::Result<String> {
        fs::read_to_string(self.dir.join(file))
    }

    pub(crate) fn seed(&self, sql: &str) -> Fallible<()> {
        Ok(Connection::open(self.database())?.execute_batch(sql)?)
    }

    pub(crate) fn write(&self, file: &str, body: &str) -> io::Result<()> {
        fs::write(self.dir.join(file), body)
    }
}

pub(crate) fn barred(ident: &str) -> String {
    json!({
        "type": "user_im_message_receive_o2o_all",
        "message_id": ident,
        "conversation_id": "c1",
        "sender": "alice",
        "sender_open_dingtalk_id": "u1",
        "content": "a || b",
        "create_time": "2026-07-08 12:00:35",
    })
    .to_string()
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

pub(crate) fn event(ident: &str) -> String {
    json!({
        "type": "user_im_message_receive_o2o_all",
        "message_id": ident,
        "conversation_id": "c1",
        "sender": "alice",
        "sender_open_dingtalk_id": "u1",
        "content": "hello",
        "create_time": "2026-07-08 12:00:35",
    })
    .to_string()
}

pub(crate) fn feed(lines: &[String]) -> String {
    format!("{}\n", lines.join("\n"))
}

pub(crate) fn fragment_event(ident: &str) -> String {
    json!({
        "type": "user_im_message_receive_group_all",
        "message_id": ident,
        "conversation_id": "c1",
        "sender": "alice",
        "sender_open_dingtalk_id": "u1",
        "content": FRAGMENTS,
        "create_time": "2026-07-08 12:00:35",
    })
    .to_string()
}

pub(crate) fn fragment_page(ids: &[&str]) -> String {
    let messages: Vec<Value> = ids
        .iter()
        .map(|ident| {
            json!({
                "messageId": ident,
                "sender": "bob",
                "content": if ident.starts_with('p') { "plain" } else { FRAGMENTS },
                "createTime": "2026-07-08 12:00:35",
            })
        })
        .collect();
    json!({
        "result": {
            "conversationMessagesList": [{"openConversationId": "c1", "messages": messages}],
            "nextCursor": null,
        }
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

pub(crate) fn page(ids: &[&str], cursor: Option<&str>) -> String {
    let messages: Vec<Value> = ids
        .iter()
        .map(|ident| {
            json!({
                "messageId": ident,
                "sender": "bob",
                "text": "yo",
                "createTime": "2026-07-08 12:00:35",
            })
        })
        .collect();
    json!({
        "result": {
            "conversationMessagesList": [{"openConversationId": "c1", "messages": messages}],
            "nextCursor": cursor,
        }
    })
    .to_string()
}

pub(crate) async fn run_for(config: &Settings, millis: u64) -> Fallible<u8> {
    Ok(run_logged(config, millis).await?.0)
}

pub(crate) async fn run_logged(config: &Settings, millis: u64) -> Fallible<(u8, String)> {
    let _: &Dispatch = LazyLock::force(&ANCHOR);
    let capture = Capture::default();
    let subscriber = fmt()
        .json()
        .with_max_level(Level::DEBUG)
        .with_writer(capture.clone())
        .finish();
    let errors: Errors = from_str(ERRORS)?;
    let paging: Paging = from_str(PAGING)?;
    let shelf = config.shelf();
    let context = Context::new(&errors, &paging, &shelf);
    let mut stop = Box::pin(sleep(Duration::from_millis(millis)));
    let code = run(config, &context, &mut stop)
        .with_subscriber(subscriber)
        .await;
    Ok((code, capture.text()))
}

pub(crate) fn sealed(ident: &str) -> String {
    json!({
        "type": "user_im_message_receive_o2o_all",
        "message_id": ident,
        "conversation_id": "c1",
        "sender": "alice",
        "sender_open_dingtalk_id": "u1",
        "content": SEALED,
        "create_time": "2026-07-08 12:00:35",
    })
    .to_string()
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
