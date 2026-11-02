use alloc::sync::Arc;
use std::io::{self, Write};

use domain::redact::Redactor;
use serde_json::{Value, from_str};
use tracing::Dispatch;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::fmt::MakeWriter;

const HIDDEN: &str = "[hidden]";
const PLAIN: [&str; 4] = ["addr", "level", "target", "timestamp"];

#[derive(Clone)]
struct Scrub {
    redactor: Option<Arc<Redactor>>,
}

impl Scrub {
    fn clean(&self, value: Value, key: &str) -> Value {
        match value {
            Value::String(text) if PLAIN.contains(&key) => Value::String(text),
            Value::String(text) => Value::String(
                self.redactor
                    .as_ref()
                    .map_or_else(|| HIDDEN.to_owned(), |redactor| redactor.apply(&text)),
            ),
            Value::Object(map) => Value::Object(
                map.into_iter()
                    .map(|(name, item)| {
                        let cleaned = self.clean(item, &name);
                        (name, cleaned)
                    })
                    .collect(),
            ),
            other @ (Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_)) => other,
        }
    }
}

impl<'writer> MakeWriter<'writer> for Scrub {
    type Writer = Self;

    fn make_writer(&'writer self) -> Self::Writer {
        self.clone()
    }
}

impl Write for Scrub {
    fn flush(&mut self) -> io::Result<()> {
        io::stderr().flush()
    }

    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let line = String::from_utf8_lossy(buf);
        let fallback = Value::String(line.to_string());
        let parsed = from_str::<Value>(line.trim_end()).unwrap_or(fallback);
        io::stderr()
            .write_all(format!("{}\n", self.clean(parsed, "")).as_bytes())
            .and_then(|()| self.flush())
            .map(|()| buf.len())
    }
}

pub(crate) fn dispatch(level: LevelFilter, redactor: Option<Redactor>) -> Dispatch {
    Dispatch::new(
        fmt()
            .json()
            .with_max_level(level)
            .with_writer(Scrub {
                redactor: redactor.map(Arc::new),
            })
            .finish(),
    )
}
