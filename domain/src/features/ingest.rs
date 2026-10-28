mod cipher;
mod stamp;

use core::iter::once;

use chrono::DateTime;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, from_str};

use self::stamp::{Stamp, offset_millis};
use crate::AppError;

const CONVERSATION_KEYS: [&str; 3] = ["conversation_id", "conversationId", "openConversationId"];
const ID_KEYS: [&str; 4] = ["message_id", "messageId", "openMessageId", "msgId"];
const SENDER_ID_KEYS: [&str; 4] = [
    "sender_open_dingtalk_id",
    "senderId",
    "senderUserId",
    "senderOpenDingTalkId",
];
const SENDER_KEYS: [&str; 2] = ["sender", "senderName"];
const STAMP_KEYS: [&str; 2] = ["create_time", "createTime"];
const TEXT_KEYS: [&str; 2] = ["content", "text"];
const WINDOW_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "IngestAssembleRequest")]
pub(crate) struct AssembleRequest {
    children: Vec<Child>,
    envelope: Envelope,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "IngestChild")]
pub(crate) struct Child {
    sender: String,
    text: String,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "IngestCipherRequest")]
pub(crate) struct CipherRequest {
    envelope: Envelope,
    min_length: u32,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "IngestCipherResponse")]
pub(crate) struct CipherResponse {
    cipher: Option<String>,
}

struct Entry {
    conversation_id: String,
    create_time: Stamp,
    message_id: String,
    raw: String,
    sender_id: String,
    sender_name: String,
    text: String,
}

impl Entry {
    fn from_map(map: &Map<String, Value>, raw: String, fallback: &str) -> Option<Self> {
        Some(Self {
            conversation_id: pick(map, &CONVERSATION_KEYS).unwrap_or_else(|| fallback.to_owned()),
            create_time: STAMP_KEYS
                .iter()
                .find_map(|key| map.get(*key))
                .and_then(Stamp::from_value)?,
            message_id: pick(map, &ID_KEYS)?,
            raw,
            sender_id: pick(map, &SENDER_ID_KEYS).unwrap_or_default(),
            sender_name: pick(map, &SENDER_KEYS).unwrap_or_default(),
            text: pick(map, &TEXT_KEYS).unwrap_or_default(),
        })
    }
}

#[derive(Deserialize, JsonSchema, Serialize)]
#[schemars(rename = "IngestEnvelope")]
pub(crate) struct Envelope {
    account: String,
    conversation_id: String,
    key: String,
    lookup: Option<String>,
    occurred_at_ms: u64,
    raw: String,
    received_at_ms: u64,
    sender_id: String,
    sender_name: String,
    source: String,
    text: String,
}

#[derive(Deserialize)]
struct Group {
    #[serde(default)]
    messages: Vec<Value>,
    #[serde(rename = "openConversationId")]
    open_conversation_id: String,
}

#[derive(Deserialize)]
struct Listing {
    #[serde(default, rename = "conversationMessagesList")]
    groups: Vec<Group>,
    #[serde(default, rename = "hasMore")]
    has_more: Option<bool>,
    #[serde(default, rename = "nextCursor")]
    next_cursor: Option<Value>,
}

#[derive(Deserialize)]
struct Page {
    result: Listing,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "IngestParsePageResponse")]
pub(crate) struct PageResponse {
    envelopes: Vec<Envelope>,
    next_cursor: Option<String>,
    skipped: u64,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "IngestParseRequest")]
pub(crate) struct ParseRequest {
    account: String,
    body: String,
    cipher_min_length: u32,
    received_at_ms: u64,
    source: String,
    utc_offset_minutes: i16,
}

impl ParseRequest {
    fn envelope(&self, entry: Entry) -> Result<Envelope, AppError> {
        let min_length = usize::try_from(self.cipher_min_length).unwrap_or(usize::MAX);
        let lookup = cipher::fragments(&entry.text, min_length).then(|| entry.message_id.clone());
        Ok(Envelope {
            account: self.account.clone(),
            conversation_id: entry.conversation_id,
            key: format!("{}:{}:{}", self.source, self.account, entry.message_id),
            lookup,
            occurred_at_ms: entry.create_time.millis(self.utc_offset_minutes)?,
            raw: entry.raw,
            received_at_ms: self.received_at_ms,
            sender_id: entry.sender_id,
            sender_name: entry.sender_name,
            source: self.source.clone(),
            text: entry.text,
        })
    }

    fn read(&self, value: &Value, fallback: &str) -> Option<Envelope> {
        let entry = value
            .as_object()
            .and_then(|map| Entry::from_map(map, value.to_string(), fallback))?;
        self.envelope(entry).ok()
    }
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "IngestRevealRequest")]
pub(crate) struct RevealRequest {
    envelope: Envelope,
    plaintext: String,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "IngestWindowRequest")]
pub(crate) struct WindowRequest {
    from_ms: u64,
    to_ms: u64,
    utc_offset_minutes: i16,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "IngestWindowResponse")]
pub(crate) struct WindowResponse {
    end: String,
    start: String,
}

pub(crate) fn assemble(req: AssembleRequest) -> Envelope {
    let title = req
        .envelope
        .text
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned();
    let lines = req
        .children
        .iter()
        .map(|child| format!("{}:{}", child.sender, child.text));
    Envelope {
        lookup: None,
        text: once(title).chain(lines).collect::<Vec<_>>().join("\n"),
        ..req.envelope
    }
}

fn bad<T>(_err: T) -> AppError {
    AppError::BadRequest
}

pub(crate) fn cipher(req: CipherRequest) -> CipherResponse {
    let min_length = usize::try_from(req.min_length).unwrap_or(usize::MAX);
    CipherResponse {
        cipher: Some(req.envelope.text).filter(|text| cipher::sealed(text, min_length)),
    }
}

fn clock(millis: u64, offset_minutes: i16) -> Result<String, AppError> {
    let shifted = i64::try_from(millis)
        .unwrap_or(i64::MAX)
        .saturating_add(offset_millis(offset_minutes));
    let stamp = DateTime::from_timestamp_millis(shifted).ok_or(AppError::BadRequest)?;
    Ok(stamp.format(WINDOW_FORMAT).to_string())
}

pub(crate) fn parse_event(req: &ParseRequest) -> Result<Envelope, AppError> {
    let map: Map<String, Value> = from_str(&req.body).map_err(bad)?;
    let entry = Entry::from_map(&map, req.body.clone(), "").ok_or(AppError::BadRequest)?;
    req.envelope(entry)
}

pub(crate) fn parse_page(req: &ParseRequest) -> Result<PageResponse, AppError> {
    let page: Page = from_str(&req.body).map_err(bad)?;
    let mut envelopes = Vec::new();
    let mut skipped: u64 = 0;
    for group in &page.result.groups {
        for value in &group.messages {
            match req.read(value, &group.open_conversation_id) {
                Some(envelope) => envelopes.push(envelope),
                None => skipped = skipped.saturating_add(1),
            }
        }
    }
    let more = page.result.has_more == Some(true);
    Ok(PageResponse {
        envelopes,
        next_cursor: page.result.next_cursor.filter(|_| more).map(|cursor| {
            cursor
                .as_str()
                .map_or_else(|| cursor.to_string(), str::to_owned)
        }),
        skipped,
    })
}

fn pick(map: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| map.get(*key).and_then(Value::as_str))
        .map(str::to_owned)
}

pub(crate) fn reveal(req: RevealRequest) -> Envelope {
    Envelope {
        text: req.plaintext,
        ..req.envelope
    }
}

pub(crate) fn window(req: &WindowRequest) -> Result<WindowResponse, AppError> {
    Ok(WindowResponse {
        end: clock(req.to_ms, req.utc_offset_minutes)?,
        start: clock(req.from_ms, req.utc_offset_minutes)?,
    })
}
