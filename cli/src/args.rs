use std::path::{Path, PathBuf};

use clap::{Args, Parser};
use domain::features::{Call, Origin, Target};

#[derive(Parser)]
pub(crate) enum Command {
    Call(Box<Invocation>),
    Serve {
        #[arg(long)]
        config: PathBuf,
    },
    Spec,
}

#[derive(Args)]
pub(crate) struct Invocation {
    #[arg(index = 2)]
    action: String,
    #[arg(long)]
    config: PathBuf,
    #[arg(index = 1)]
    feature: String,
    #[arg(long)]
    json: String,
    #[arg(long)]
    session_id: Option<String>,
    #[arg(long)]
    user: Option<String>,
}

impl Invocation {
    pub(crate) fn call(&self) -> Call<'_> {
        Call::new(
            Origin::new("cli", self.session_id.as_deref(), self.user.as_deref()),
            Target::new(&self.feature, &self.action),
            &self.json,
        )
    }

    pub(crate) fn config(&self) -> &Path {
        &self.config
    }
}
