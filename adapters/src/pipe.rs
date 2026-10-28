use core::future::poll_fn;
use core::task::Poll;
use core::time::Duration;
use std::io;
use std::process::{Output, Stdio};

use tokio::io::AsyncWriteExt as _;
use tokio::process::Command;
use tokio::time::timeout;

use crate::triage::config::Exec;

#[derive(Clone, Copy)]
pub(crate) struct Job<'job> {
    exec: &'job Exec,
    extra: &'job [String],
    input: &'job str,
}

impl<'job> Job<'job> {
    pub(crate) const fn new(exec: &'job Exec, extra: &'job [String], input: &'job str) -> Self {
        Self { exec, extra, input }
    }
}

fn bare(exec: &Exec) -> Command {
    let mut command = exec.sandbox().command(exec.bin());
    let _: &mut Command = command.env_clear().envs(exec.env()).envs(exec.vars());
    command
}

pub(crate) async fn feed(job: Job<'_>) -> io::Result<Output> {
    let mut command = bare(job.exec);
    let mut child = command
        .args(job.exec.args())
        .args(job.extra)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let mut writing = child
        .stdin
        .take()
        .map(|mut pipe| Box::pin(async move { pipe.write_all(job.input.as_bytes()).await }));
    let limit = Duration::from_millis(job.exec.timeout_ms());
    timeout(limit, async {
        drop(
            poll_fn(|context| {
                writing
                    .as_mut()
                    .map_or(Poll::Ready(Ok(())), |work| work.as_mut().poll(context))
            })
            .await,
        );
        child.wait_with_output().await
    })
    .await
    .unwrap_or_else(|_elapsed| Err(io::Error::from(io::ErrorKind::TimedOut)))
}

pub(crate) async fn probe(exec: &Exec) -> Result<(), String> {
    exec.sandbox().check(bare(exec), exec.bin()).await
}
