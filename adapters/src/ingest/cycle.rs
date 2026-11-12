use rusqlite::{Error, ErrorCode};

use crate::config::Settings;
use crate::jitter::spread;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Backoff {
    delay: u64,
    initial: u64,
    max: u64,
    streak: u32,
}

impl Backoff {
    pub(crate) const fn new(initial: u64, max: u64) -> Self {
        Self {
            delay: initial,
            initial,
            max,
            streak: 0,
        }
    }

    pub(crate) fn next(&mut self, retry: Retry) -> u64 {
        let (wait, streak) = match retry {
            Retry::Failed => (self.delay, self.streak.saturating_add(1)),
            Retry::Progress => (self.initial, 1),
            Retry::Stalled => (self.max, self.streak.saturating_add(1)),
        };
        self.delay = wait.saturating_mul(2).min(self.max);
        self.streak = streak;
        spread(wait)
    }

    pub(crate) const fn streak(&self) -> u32 {
        self.streak
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
