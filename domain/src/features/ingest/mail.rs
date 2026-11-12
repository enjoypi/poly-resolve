use chrono::DateTime;
use mail_parser::{Mailbox, Message, MessageParser, MessagePart};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::Envelope;
use crate::AppError;

const AUTO: &str = "auto";
const BULK: [&str; 3] = ["bulk", "junk", "list"];
const DMARC: &str = "dmarc";
const DMARC_FAIL: &str = "dmarc=fail";
const INTERNAL_FORMAT: &str = "%d-%b-%Y %H:%M:%S %z";
const LIST: &str = "list";
const SINCE_FORMAT: &str = "%-d-%b-%Y";

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "IngestMailLimits")]
pub(crate) struct Limits {
    chars: u32,
    depth: u32,
    parts: u32,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "IngestMailRequest")]
pub(crate) struct MailRequest {
    account: String,
    internal: String,
    limits: Limits,
    raw: Vec<u8>,
    received_at_ms: u64,
    source: String,
    style: Style,
    uid: u32,
    uid_validity: u32,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "IngestMailResponse")]
pub(crate) struct MailResponse {
    envelope: Envelope,
    skip: Option<String>,
}

#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[schemars(rename = "IngestSinceRequest")]
pub(crate) struct SinceRequest {
    at_ms: u64,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "IngestSinceResponse")]
pub(crate) struct SinceResponse {
    date: String,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "IngestMailStyle")]
pub(crate) struct Style {
    attachment: String,
    quote_ends: Vec<String>,
    quote_starts: Vec<String>,
}

fn attachments(message: &Message<'_>, style: &Style) -> Vec<String> {
    message
        .attachments()
        .map(|part| {
            style
                .attachment
                .replacen("{type}", &kind(&part), 1)
                .replacen(
                    "{name}",
                    &clean(part.attachment_name().unwrap_or_default()),
                    1,
                )
        })
        .collect()
}

fn body(message: &Message<'_>, style: &Style) -> String {
    let text = message.body_text(0).unwrap_or_default();
    let mut kept = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if quoted(trimmed, style) {
            break;
        }
        if !trimmed.starts_with('>') {
            kept.push(line);
        }
    }
    kept.join("\n").trim().to_owned()
}

fn bulk(message: &Message<'_>) -> Option<&'static str> {
    if header(message, "Auto-Submitted").is_some_and(|found| found != "no") {
        Some(AUTO)
    } else if header(message, "List-Id").is_some()
        || header(message, "Precedence").is_some_and(|found| BULK.contains(&found.as_str()))
    {
        Some(LIST)
    } else {
        None
    }
}

fn clean(text: &str) -> String {
    text.chars().filter(|letter| !letter.is_control()).collect()
}

fn header(message: &Message<'_>, name: &str) -> Option<String> {
    message.headers().all(name).next().map(|found| {
        String::from_utf8_lossy(found.raw_value())
            .trim()
            .to_lowercase()
    })
}

fn kind(part: &MessagePart<'_>) -> String {
    part.content_type().map_or_default(|found| {
        found.subtype().map_or_else(
            || found.ctype().to_owned(),
            |sub| format!("{}/{sub}", found.ctype()),
        )
    })
}

pub(crate) fn parse_mail(req: &MailRequest) -> Result<MailResponse, AppError> {
    let limits = &req.limits;
    let message = MessageParser::new()
        .max_depth(usize::try_from(limits.depth).unwrap_or(usize::MAX))
        .max_parts(usize::try_from(limits.parts).unwrap_or(usize::MAX))
        .parse(&req.raw)
        .ok_or(AppError::BadRequest)?;
    let max = usize::try_from(limits.chars).unwrap_or(usize::MAX);
    let occurred = DateTime::parse_from_str(req.internal.trim(), INTERNAL_FORMAT)
        .ok()
        .and_then(|stamp| u64::try_from(stamp.timestamp_millis()).ok())
        .unwrap_or(req.received_at_ms);
    let sender = message.from().and_then(|list| list.first());
    let address = sender
        .as_ref()
        .and_then(Mailbox::address)
        .unwrap_or_default()
        .to_lowercase();
    let name = clean(sender.as_ref().and_then(Mailbox::name).unwrap_or_default());
    let ident = message.message_id().map_or_else(
        || format!("uid:{}:{}", req.uid_validity, req.uid),
        str::to_owned,
    );
    let mut parts = vec![
        clean(message.subject().unwrap_or_default()),
        body(&message, &req.style),
    ];
    parts.extend(attachments(&message, &req.style));
    Ok(MailResponse {
        envelope: Envelope {
            account: req.account.clone(),
            conversation_id: format!("mail:{}:{address}", req.account),
            key: format!("{}:{}:{ident}", req.source, req.account),
            lookup: None,
            occurred_at_ms: occurred,
            raw: summary(&message, req).to_string(),
            received_at_ms: req.received_at_ms,
            sender_id: address,
            sender_name: name.chars().take(max).collect(),
            source: req.source.clone(),
            text: parts.join("\n").trim().chars().take(max).collect(),
        },
        skip: skip(&message),
    })
}

fn quoted(line: &str, style: &Style) -> bool {
    style
        .quote_starts
        .iter()
        .any(|start| line.starts_with(start.as_str()))
        || style
            .quote_ends
            .iter()
            .any(|end| line.ends_with(end.as_str()))
}

pub(crate) fn since(req: SinceRequest) -> SinceResponse {
    let SinceRequest { at_ms } = req;
    let stamp = DateTime::from_timestamp_millis(i64::try_from(at_ms).unwrap_or(i64::MAX))
        .unwrap_or_default();
    SinceResponse {
        date: stamp.format(SINCE_FORMAT).to_string(),
    }
}

fn skip(message: &Message<'_>) -> Option<String> {
    header(message, "Authentication-Results")
        .filter(|found| found.contains(DMARC_FAIL))
        .map(|_| DMARC.to_owned())
}

fn summary(message: &Message<'_>, req: &MailRequest) -> Value {
    json!({
        "bulk": bulk(message),
        "date": message.date().map(|stamp| stamp.to_rfc3339()),
        "from": message.from().and_then(|list| list.first()).and_then(|mailbox| mailbox.address().map(str::to_owned)),
        "in_reply_to": message.in_reply_to().and_then(|list| list.first()),
        "internal": req.internal,
        "message_id": message.message_id(),
        "references": message.references().map(|list| list.iter().collect::<Vec<_>>()),
        "size": req.raw.len(),
        "subject": message.subject(),
        "uid": req.uid,
        "uid_validity": req.uid_validity,
    })
}
