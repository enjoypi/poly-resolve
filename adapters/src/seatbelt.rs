use alloc::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::canonicalize;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};

use futures_util::future::join;
use serde::Deserialize;
use tokio::process::Command;

use crate::triage::config::Var;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sandbox {
    bin: String,
    canary: String,
    paths: BTreeMap<String, PathBuf>,
    probe: Vec<String>,
    profile: PathBuf,
    vars: BTreeMap<String, Var>,
}

impl Sandbox {
    fn arguments(&self, program: &str) -> Vec<OsString> {
        let paths = self
            .paths
            .iter()
            .map(|(name, path)| (name, real(path).into_os_string()));
        let vars = self
            .vars
            .iter()
            .map(|(name, var)| (name, OsString::from(var.value().unwrap_or_default())));
        let mut args = vec![OsString::from("-f"), real(&self.profile).into_os_string()];
        for (name, value) in paths.chain(vars) {
            let mut define = OsString::from(name);
            define.push("=");
            define.push(value);
            args.extend([OsString::from("-D"), define]);
        }
        args.extend([OsString::from("--"), OsString::from(program)]);
        args
    }

    pub(crate) async fn check(&self, mut probe: Command, name: &str) -> Result<(), String> {
        let _: &mut Command = probe.args(&self.probe);
        let (canary, allowed) = join(run(self.command(&self.canary)), run(probe)).await;
        match (canary, allowed) {
            (Err(err), _) | (_, Err(err)) => {
                Err(format!("{name}: sandbox did not start: {}", err.kind()))
            }
            (Ok(denied), _) if denied.success() => {
                Err(format!("{name}: canary {} was not denied", self.canary))
            }
            (Ok(_), Ok(ran)) if !ran.success() => Err(format!("{name}: probe failed: {ran}")),
            (Ok(_), Ok(_)) => Ok(()),
        }
    }

    pub(crate) fn command(&self, program: &str) -> Command {
        let mut command = Command::new(&self.bin);
        let _: &mut Command = command.args(self.arguments(program));
        command
    }

    pub(crate) fn vars(&self) -> impl Iterator<Item = &Var> {
        self.vars.values()
    }
}

fn real(path: &Path) -> PathBuf {
    canonicalize(path).unwrap_or_default()
}

async fn run(mut command: Command) -> io::Result<ExitStatus> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .status()
        .await
}
