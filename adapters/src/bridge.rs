use domain::AppError;
use domain::features::{Call, Context, Origin, Target, dispatch};
use serde::Deserialize;
use serde_json::{from_str, json};

use crate::config::Settings;

const FEATURE: &str = "ingest";
const TRANSPORT: &str = "adapters";

pub(crate) struct Bridge<'bridge> {
    config: &'bridge Settings,
    context: &'bridge Context<'bridge>,
}

impl<'bridge> Bridge<'bridge> {
    fn call(&self, action: &str, request: &str) -> Result<String, AppError> {
        dispatch(
            self.context,
            &Call::new(
                Origin::new(TRANSPORT, None, None),
                Target::new(FEATURE, action),
                request,
            ),
        )
    }

    pub(crate) const fn new(config: &'bridge Settings, context: &'bridge Context<'bridge>) -> Self {
        Self { config, context }
    }

    pub(crate) fn parse_event(&self, line: &str, received_at_ms: u64) -> Result<String, AppError> {
        self.call("parse_event", &self.parse_request(line, received_at_ms))
    }

    pub(crate) fn parse_page(&self, body: &str, received_at_ms: u64) -> Result<String, AppError> {
        self.call("parse_page", &self.parse_request(body, received_at_ms))
    }

    fn parse_request(&self, body: &str, received_at_ms: u64) -> String {
        json!({
            "account": self.config.account(),
            "body": body,
            "received_at_ms": received_at_ms,
            "source": self.config.source(),
            "utc_offset_minutes": self.config.source_utc_offset_minutes(),
        })
        .to_string()
    }

    pub(crate) fn window(&self, from_ms: u64, to_ms: u64) -> Result<Window, AppError> {
        let request = json!({
            "from_ms": from_ms,
            "to_ms": to_ms,
            "utc_offset_minutes": self.config.source_utc_offset_minutes(),
        });
        let response = self.call("window", &request.to_string())?;
        Ok(from_str(&response).unwrap_or_default())
    }
}

#[derive(Default, Deserialize)]
pub(crate) struct Summary {
    next_cursor: Option<String>,
    skipped: u64,
}

impl Summary {
    pub(crate) const fn next_cursor(&self) -> Option<&String> {
        self.next_cursor.as_ref()
    }

    pub(crate) fn parse(page: &str) -> Self {
        from_str(page).unwrap_or_default()
    }

    pub(crate) const fn skipped(&self) -> u64 {
        self.skipped
    }
}

#[derive(Default, Deserialize)]
pub(crate) struct Window {
    end: String,
    start: String,
}

impl Window {
    pub(crate) fn end(&self) -> &str {
        &self.end
    }

    pub(crate) fn start(&self) -> &str {
        &self.start
    }
}
