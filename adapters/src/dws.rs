use core::future::poll_fn;
use core::pin::Pin;
use core::task::Poll;
use core::time::Duration;
use std::io;
use std::process::{Output, Stdio};

use tokio::io::{AsyncBufReadExt as _, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time::timeout;

use crate::config::{Dws as DwsConfig, Process};

pub(crate) struct Dws<'dws> {
    config: &'dws DwsConfig,
}

impl<'dws> Dws<'dws> {
    pub(crate) fn listen(&self) -> io::Result<Listener> {
        let mut command = Command::new(self.config.listen().bin());
        let mut child = self
            .prepare(&mut command, self.config.listen())
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

    pub(crate) fn manual(&self, code: Option<i32>) -> bool {
        code.is_some_and(|value| self.config.manual_exit_codes().contains(&value))
    }

    pub(crate) const fn new(config: &'dws DwsConfig) -> Self {
        Self { config }
    }

    fn prepare<'cmd>(&self, command: &'cmd mut Command, process: &Process) -> &'cmd mut Command {
        command
            .args(process.args())
            .envs(self.config.env())
            .kill_on_drop(true)
    }

    pub(crate) async fn pull(&self, start: &str, end: &str) -> io::Result<Output> {
        let mut command = Command::new(self.config.pull().bin());
        self.prepare(&mut command, self.config.pull())
            .args(["--start", start, "--end", end])
            .stdin(Stdio::null())
            .output()
            .await
    }
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
