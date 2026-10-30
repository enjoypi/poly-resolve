use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
pub(crate) enum Command {
    Ingest {
        #[arg(long)]
        config: PathBuf,
    },
    Serve {
        #[arg(long)]
        config: PathBuf,
    },
    Spec,
}
