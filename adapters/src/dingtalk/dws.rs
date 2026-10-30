use alloc::collections::BTreeMap;
use core::future::poll_fn;
use core::pin::Pin;
use core::task::Poll;
use core::time::Duration;
use std::io::{self, ErrorKind};
use std::process::{Output, Stdio};

use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use serde_json::from_slice;
use tokio::io::{AsyncBufReadExt as _, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time::timeout;

use super::config::{Dws as DwsConfig, Process};

pub(crate) struct Dws<'dws> {
    config: &'dws DwsConfig,
}

impl<'dws> Dws<'dws> {
    fn bare(&self, process: &Process) -> Command {
        let mut command = process.sandbox().command(process.bin());
        let _: &mut Command = command
            .env_clear()
            .kill_on_drop(true)
            .envs(self.config.env())
            .envs(self.config.vars());
        command
    }

    pub(crate) async fn check(&self) -> Result<(), String> {
        let checks = [
            self.config.decrypt(),
            self.config.listen(),
            self.config.lookup(),
            self.config.pull(),
        ]
        .map(|process| process.sandbox().check(self.bare(process), process.bin()));
        join_all(checks).await.into_iter().collect()
    }

    pub(crate) async fn decrypt(&self, cipher: &str) -> io::Result<Output> {
        let mut command = self.prepare(self.config.decrypt());
        let _: &mut Command = command.arg(format!("--text={cipher}"));
        self.output(command).await
    }

    pub(crate) fn listen(&self) -> io::Result<Listener> {
        let mut child = self
            .prepare(self.config.listen())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let lines = child
            .stdout
            .take()
            .map(|stdout| BufReader::new(stdout).lines());
        let stdin = child.stdin.take();
        Ok(Listener {
            child,
            lines,
            stdin,
        })
    }

    pub(crate) async fn lookup(&self, ids: &[String]) -> io::Result<Output> {
        let mut command = self.prepare(self.config.lookup());
        let _: &mut Command = command.arg(format!("--msg-ids={}", ids.join(",")));
        self.output(command).await
    }

    pub(crate) fn manual(&self, code: Option<i32>) -> bool {
        code.is_some_and(|value| self.config.manual_exit_codes().contains(&value))
    }

    pub(crate) const fn new(config: &'dws DwsConfig) -> Self {
        Self { config }
    }

    async fn output(&self, mut command: Command) -> io::Result<Output> {
        let limit = Duration::from_millis(self.config.timeout_ms());
        timeout(limit, command.stdin(Stdio::null()).output())
            .await
            .unwrap_or_else(|_elapsed| Err(io::Error::from(ErrorKind::TimedOut)))
    }

    fn prepare(&self, process: &Process) -> Command {
        let mut command = self.bare(process);
        let _: &mut Command = command.args(process.args());
        command
    }

    pub(crate) async fn pull(&self, start: &str, end: &str) -> io::Result<Output> {
        let mut command = self.prepare(self.config.pull());
        let _: &mut Command = command.args(["--start", start, "--end", end]);
        self.output(command).await
    }
}

pub(crate) type Found = BTreeMap<String, Vec<Kid>>;

#[derive(Deserialize, Serialize)]
pub(crate) struct Kid {
    #[serde(default)]
    sender: String,
    #[serde(default)]
    text: String,
}

pub(crate) struct Listener {
    child: Child,
    lines: Option<Lines<BufReader<ChildStdout>>>,
    stdin: Option<ChildStdin>,
}

impl Listener {
    pub(crate) async fn next_line(&mut self) -> io::Result<Option<String>> {
        poll_fn(|context| {
            self.lines.as_mut().map_or(Poll::Ready(Ok(None)), |lines| {
                Pin::new(lines).poll_next_line(context)
            })
        })
        .await
    }

    pub(crate) async fn stop(&mut self, limit: Duration) -> Option<i32> {
        self.stdin = None;
        match timeout(limit, self.child.wait()).await {
            Ok(status) => status.ok().and_then(|exit| exit.code()),
            Err(_elapsed) => {
                tracing::warn!(timeout_ms = limit.as_millis(), "dws did not exit, killing");
                let _: Option<()> = self.child.kill().await.ok();
                None
            }
        }
    }
}

#[derive(Deserialize)]
struct Looked {
    #[serde(default)]
    forwarded: Vec<Kid>,
    #[serde(rename = "messageId")]
    message_id: String,
}

#[derive(Deserialize)]
struct Lookup {
    #[serde(default)]
    messages: Vec<Looked>,
}

#[derive(Deserialize)]
struct Revealed {
    plaintext: Option<String>,
}

pub(crate) fn children(output: &Output) -> Found {
    from_slice::<Lookup>(&output.stdout).map_or_default(|lookup| {
        lookup
            .messages
            .into_iter()
            .map(|looked| (looked.message_id, looked.forwarded))
            .collect()
    })
}

pub(crate) fn plaintext(output: &Output) -> Option<String> {
    let revealed: Revealed = from_slice(&output.stdout).ok()?;
    revealed.plaintext
}
