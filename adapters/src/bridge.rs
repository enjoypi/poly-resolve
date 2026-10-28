use domain::AppError;
use domain::features::{Call, Context, Origin, Target, dispatch};
use serde::{Deserialize, Serialize};
use serde_json::{Value, from_str, json, to_string};

use crate::config::Settings;
use crate::dws::Kid;

const FEATURE: &str = "ingest";
const TRANSPORT: &str = "adapters";

pub(crate) struct Bridge<'bridge> {
    config: &'bridge Settings,
    context: &'bridge Context<'bridge>,
}

impl<'bridge> Bridge<'bridge> {
    pub(crate) fn assemble(&self, envelope: &str, kids: &[Kid]) -> Option<Value> {
        let children = to_string(kids).unwrap_or_default();
        let request = format!(r#"{{"children":{children},"envelope":{envelope}}}"#);
        self.call("assemble", &request)
            .map_or_default(|reply| from_str(&reply).ok())
    }

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

    pub(crate) fn cipher(&self, envelope: &str) -> Option<String> {
        let request = format!(
            r#"{{"envelope":{envelope},"min_length":{}}}"#,
            self.config.cipher_min_length()
        );
        self.call("cipher", &request)
            .map_or_default(|reply| from_str::<Cipher>(&reply).map_or_default(|found| found.cipher))
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
            "cipher_min_length": self.config.cipher_min_length(),
            "received_at_ms": received_at_ms,
            "source": self.config.source(),
            "utc_offset_minutes": self.config.source_utc_offset_minutes(),
        })
        .to_string()
    }

    pub(crate) fn reveal(&self, envelope: &str, plaintext: &str) -> Result<String, AppError> {
        let quoted = to_string(plaintext).unwrap_or_default();
        self.call(
            "reveal",
            &format!(r#"{{"envelope":{envelope},"plaintext":{quoted}}}"#),
        )
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

#[derive(Deserialize)]
struct Cipher {
    cipher: Option<String>,
}

#[derive(Default, Deserialize, Serialize)]
pub(crate) struct Page {
    envelopes: Vec<Value>,
    next_cursor: Option<String>,
    skipped: u64,
}

impl Page {
    pub(crate) fn envelopes_mut(&mut self) -> &mut [Value] {
        &mut self.envelopes
    }

    pub(crate) const fn next_cursor(&self) -> Option<&String> {
        self.next_cursor.as_ref()
    }

    pub(crate) fn parse(page: &str) -> Self {
        from_str(page).unwrap_or_default()
    }

    pub(crate) fn render(&self) -> String {
        to_string(self).unwrap_or_default()
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
