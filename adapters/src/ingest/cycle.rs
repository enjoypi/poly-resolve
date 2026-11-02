use rusqlite::{Error, ErrorCode};

use crate::config::Settings;
use crate::jitter::spread;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Backoff {
    delay: u64,
}

impl Backoff {
    pub(crate) const fn new(shared: &Settings) -> Self {
        Self {
            delay: shared.backoff_initial_ms(),
        }
    }

    pub(crate) fn next(&mut self, shared: &Settings, retry: Retry) -> u64 {
        let max = shared.backoff_max_ms();
        let wait = match retry {
            Retry::Failed => self.delay,
            Retry::Progress => shared.backoff_initial_ms(),
            Retry::Stalled => max,
        };
        self.delay = wait.saturating_mul(2).min(max);
        spread(wait)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Halt {
    Busy,
    Exit(u8),
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Retry {
    Failed,
    Progress,
    Stalled,
}

pub(crate) fn fault(shared: &Settings, err: &Error) -> Halt {
    if matches!(
        err.sqlite_error_code(),
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked)
    ) {
        tracing::warn!(error = %err, "store busy");
        Halt::Busy
    } else {
        tracing::error!(error = %err, "store failed");
        Halt::Exit(shared.failure_exit_code())
    }
}
