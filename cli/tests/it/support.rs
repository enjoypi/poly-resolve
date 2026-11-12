use std::ffi::OsString;
use std::fs;
use std::io::{self, BufReader, Lines};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, ChildStderr, Command, ExitCode, Output};
use std::sync::OnceLock;

use cli::run as run_cli;

pub(crate) const BUSY: u64 = 5_000;
pub(crate) const EVENT: &str = r#"{"message_id":"m1","conversation_id":"c1","sender":"alice","sender_open_dingtalk_id":"u1","content":"hello","create_time":"2026-07-08 12:00:35"}"#;
const FAKE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../adapters/tests/it/fixtures/fake-dws.sh"
);
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../adapters/tests/it/fixtures");
const PROFILE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../adapters/tests/it/fixtures/fake.sb"
);
const PROMPTS: [(&str, &str); 4] = [
    ("followup", "f {messages}"),
    ("session", "p {messages}"),
    ("topic", "t {segment}"),
    ("urgent", "i {line}"),
];
pub(crate) const TRIAGE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../adapters/tests/it/fixtures/fake-triage.sh"
);
pub(crate) const YAML: &str = "addr: \"127.0.0.1:0\"
log_level: debug
sandbox_exit_code: 18
serve_exit_code: 9
shutdown_timeout_exit_code: 8
shutdown_timeout_ms: 10000
web_dir: /nonexistent/web
messages:
  default: 2
  max: 3
errors:
  bad_request:
    code: invalid
  conflict:
    code: clash
  not_found:
    code: absent
  storage:
    code: unavailable
  unknown_action:
    code: missing
  unprocessable:
    code: refused
triage:
  codex: {args: [x], bin: /nonexistent/triage, env: {FAKE_DIR: /nonexistent}, sandbox: &fence {bin: /usr/bin/sandbox-exec, canary: /usr/bin/true, paths: {DIR: /nonexistent/sandbox-dir, FIXTURES: /nonexistent/fixtures}, probe: [noop], profile: /nonexistent/fake.sb, vars: {HOME: \"${HOME}\"}}, timeout_ms: 1000, vars: {PATH: \"${PATH}\"}}
  decide: {key: \"${CARGO_PKG_NAME}\", timeout_ms: 1000, url: \"http://localhost:15801/decisions\"}
  limits:
    batch: 1
    intake: 5
    lookback_ms: 1000
    max_attempts: 1
    max_revivals: 1
    parallel: 2
    poll_ms: 10
    retry_after_ms: 0
    retry_max_ms: 1000
  mode: disabled
  model: m
  notice: {late: l, late_after_ms: 1000, states: {}, template: n}
  notify: {args: [x], bin: /nonexistent/triage, env: {FAKE_DIR: /nonexistent}, sandbox: *fence, timeout_ms: 1000, vars: {PATH: \"${PATH}\"}}
  notify_target: \"${HOME}\"
  prompts_dir: /nonexistent/prompts
  questions: {threshold: 0.8, topic: {min_confidence: 0.6, name: topic, new_label: n, none_label: z}, urgent: {name: urgent}}
  redact:
    patterns:
      - pattern: \"[0-9]{11,}\"
        placeholder: \"[num]\"
    unknown_sender: unknown
  session: {fresh: [\"-\"], omitted: o, resume: [resume, \"{thread}\", \"-\"]}
  sizes:
    context_max_chars: 1000
    label_chars: 10
    result_max_chars: 100
    session_gap_ms: 1000
  topics:
    archive_after_ms: 1000
    clues: {patterns: [], subject: {ignore: [], min_chars: 4, prefixes: []}}
    max_open: 5
    reach: {cap: 0.5, conversations: 0.1, people: 0.1, sources: 0.1}
ingest:
  backoff_initial_ms: 5
  backoff_max_ms: 20
  busy_timeout_ms: 5000
  failure_exit_code: 16
  manual_exit_code: 17
  sources:
    - dingtalk:
        account: acct
        child_exit_timeout_ms: 300
        cipher_min_length: 32
        dws:
          decrypt:
            args: [decrypt]
            bin: /nonexistent/dws
            sandbox: *fence
          env:
            FAKE_DIR: /nonexistent
          listen:
            args: [listen]
            bin: /nonexistent/dws
            sandbox: *fence
          lookup:
            args: [lookup]
            bin: /nonexistent/dws
            sandbox: *fence
          manual_exit_codes: [2, 4]
          parallel: 1
          pull:
            args: [pull]
            bin: /nonexistent/dws
            sandbox: *fence
          timeout_ms: 30000
          vars:
            PATH: \"${PATH}\"
        heal_limit: 50
        initial_backfill_ms: 3600000
        lookup_batch: 50
        notify_conversation_id: \"${HOME}\"
        reconcile_overlap_ms: 1000
        self_sender_id: \"${HOME}\"
        utc_offset_minutes: 480
  store_path: /nonexistent/dir/store.db
";

pub(crate) type Stderr = Lines<BufReader<ChildStderr>>;

pub(crate) fn await_log(lines: &mut Stderr, needle: &str) -> io::Result<()> {
    for line in lines {
        if line?.contains(needle) {
            return Ok(());
        }
    }
    Err(io::Error::other(format!("log not found: {needle}")))
}

pub(crate) fn config(name: &str, body: &str) -> io::Result<String> {
    let path = format!("{}/{name}", env!("CARGO_TARGET_TMPDIR"));
    fs::write(&path, body)?;
    Ok(path)
}

pub(crate) fn decisions() -> String {
    static OPEN: OnceLock<Option<TcpListener>> = OnceLock::new();
    OPEN.get_or_init(|| TcpListener::bind("127.0.0.1:0").ok())
        .as_ref()
        .and_then(|listener| listener.local_addr().ok())
        .map_or_else(String::new, |address| address.to_string())
}

pub(crate) fn live(dir: &str) -> String {
    drop(prompts(dir));
    YAML.replace("/nonexistent/prompts", &format!("{dir}/prompts"))
        .replace("localhost:15801", &decisions())
        .replace("/nonexistent/dws", FAKE)
        .replace("/nonexistent/triage", TRIAGE)
        .replace("/nonexistent/fake.sb", PROFILE)
        .replace("/nonexistent/fixtures", FIXTURES)
        .replace("/nonexistent/sandbox-dir", dir)
        .replace("FAKE_DIR: /nonexistent", &format!("FAKE_DIR: {dir}"))
        .replace("/nonexistent/dir/store.db", &format!("{dir}/store.db"))
}

pub(crate) async fn load(config: &str) -> ExitCode {
    run(&["cli", "ingest", "--config", config]).await
}

fn prompts(dir: &str) -> io::Result<()> {
    let folder = format!("{dir}/prompts");
    fs::create_dir_all(&folder)?;
    for (name, text) in PROMPTS {
        fs::write(format!("{folder}/{name}.md"), text)?;
    }
    Ok(())
}

pub(crate) async fn run(args: &[&str]) -> ExitCode {
    run_cli(args.iter().map(OsString::from).collect()).await
}

pub(crate) fn sandbox(name: &str) -> io::Result<String> {
    let dir = format!("{}/{name}", env!("CARGO_TARGET_TMPDIR"));
    if Path::new(&dir).exists() {
        fs::remove_dir_all(&dir)?;
    }
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub(crate) fn spec() -> io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_cli")).arg("spec").output()
}

pub(crate) fn terminate(child: &Child, signal: &str) -> io::Result<()> {
    let status = Command::new("kill")
        .args([signal, &child.id().to_string()])
        .status()?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| io::Error::other("kill failed"))
}
