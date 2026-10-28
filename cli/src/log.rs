use std::io;

use tracing::Dispatch;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt;

pub(crate) fn dispatch(level: LevelFilter) -> Dispatch {
    Dispatch::new(
        fmt()
            .json()
            .with_max_level(level)
            .with_writer(io::stderr)
            .finish(),
    )
}
