use std::ffi::OsString;
use std::fs;
use std::io::{self, BufReader, Lines};
use std::path::Path;
use std::process::{Child, ChildStderr, Command, ExitCode, Output};

use cli::run as run_cli;

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
    exit_code: 13
  storage:
    code: unavailable
    exit_code: 15
  unknown_action:
    code: missing
    exit_code: 14
triage:
  codex: {args: [x], bin: /nonexistent/triage, env: {FAKE_DIR: /nonexistent}, sandbox: &fence {bin: /usr/bin/sandbox-exec, canary: /usr/bin/true, paths: {DIR: /nonexistent/sandbox-dir, FIXTURES: /nonexistent/fixtures}, probe: [noop], profile: /nonexistent/fake.sb, vars: {HOME: \"${HOME}\"}}, timeout_ms: 1000, vars: {PATH: \"${PATH}\"}}
  decide: {args: [x], bin: /nonexistent/triage, env: {FAKE_DIR: /nonexistent}, sandbox: *fence, timeout_ms: 1000, vars: {PATH: \"${PATH}\"}}
  limits:
    batch: 1
    handoff_cooldown_ms: 0
    intake: 5
    lookback_ms: 1000
    max_attempts: 1
    max_decisions_per_hour: 1
    max_handoffs_per_hour: 1
    poll_ms: 10
    retry_after_ms: 0
  mode: disabled
  model: m
  notice: n
  notify: {args: [x], bin: /nonexistent/triage, env: {FAKE_DIR: /nonexistent}, sandbox: *fence, timeout_ms: 1000, vars: {PATH: \"${PATH}\"}}
  notify_conversation_id: \"${HOME}\"
  notify_target: \"${HOME}\"
  questions: {threshold: 0.8, topic: {instructions: t, min_confidence: 0.6, name: topic, new_label: n}, urgent: {instructions: i, name: urgent}}
  redact:
    patterns:
      - pattern: \"[0-9]{11,}\"
        placeholder: \"[num]\"
    unknown_sender: unknown
  review: {learn_min_chars: 4, mode: manual, reasons: [{code: leak, label: Leaked, placeholder: \"[x]\"}, {code: noise, label: Noise}]}
  self_sender_id: \"${HOME}\"
  session: {followup: f, fresh: [\"-\"], prompt: p, resume: [resume, \"{thread}\", \"-\"]}
  sizes:
    context_max_chars: 1000
    label_chars: 10
    result_max_chars: 100
    session_gap_ms: 1000
    session_max_ms: 1000
    topics_max_open: 5
ingest:
  account: acct
  backoff_initial_ms: 5
  backoff_max_ms: 20
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
    pull:
      args: [pull]
      bin: /nonexistent/dws
      sandbox: *fence
    vars:
      PATH: \"${PATH}\"
  failure_exit_code: 16
  heal_limit: 50
  initial_backfill_ms: 3600000
  lookup_batch: 50
  manual_exit_code: 17
  reconcile_overlap_ms: 1000
  source: dingtalk
  source_utc_offset_minutes: 480
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

pub(crate) async fn call(config: &str, target: [&str; 2], json: &str) -> ExitCode {
    let [feature, action] = target;
    run(&[
        "cli", "call", feature, action, "--config", config, "--json", json,
    ])
    .await
}

pub(crate) fn config(name: &str, body: &str) -> io::Result<String> {
    let path = format!("{}/{name}", env!("CARGO_TARGET_TMPDIR"));
    fs::write(&path, body)?;
    Ok(path)
}

pub(crate) fn live(dir: &str) -> String {
    YAML.replace("/nonexistent/dws", FAKE)
        .replace("/nonexistent/triage", TRIAGE)
        .replace("/nonexistent/fake.sb", PROFILE)
        .replace("/nonexistent/fixtures", FIXTURES)
        .replace("/nonexistent/sandbox-dir", dir)
        .replace("FAKE_DIR: /nonexistent", &format!("FAKE_DIR: {dir}"))
        .replace("/nonexistent/dir/store.db", &format!("{dir}/store.db"))
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
