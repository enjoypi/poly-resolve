extern crate alloc;

use alloc::sync::Arc;
use std::io;
use std::sync::{LazyLock, Mutex, PoisonError};

use tracing::subscriber::NoSubscriber;
use tracing::subscriber::with_default;
use tracing::{Dispatch, Level};
use tracing_subscriber::fmt;
use tracing_subscriber::fmt::MakeWriter;

static ANCHOR: LazyLock<Dispatch> = LazyLock::new(|| Dispatch::new(NoSubscriber::default()));

#[derive(Clone, Default)]
pub(crate) struct Capture(Arc<Mutex<Vec<u8>>>);

impl Capture {
    pub(crate) fn record<T, F>(&self, call: F) -> T
    where
        F: FnOnce() -> T,
    {
        let _: &Dispatch = LazyLock::force(&ANCHOR);
        let subscriber = fmt()
            .json()
            .with_max_level(Level::DEBUG)
            .with_writer(self.clone())
            .finish();
        with_default(subscriber, call)
    }

    pub(crate) fn text(&self) -> String {
        let bytes = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        String::from_utf8(bytes).unwrap_or_default()
    }
}

impl<'writer> MakeWriter<'writer> for Capture {
    type Writer = Self;

    fn make_writer(&'writer self) -> Self::Writer {
        self.clone()
    }
}

impl io::Write for Capture {
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend_from_slice(buf);
        Ok(buf.len())
    }
}
