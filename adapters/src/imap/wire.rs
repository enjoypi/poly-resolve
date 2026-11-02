use core::time::Duration;

use tokio::time::{Instant, timeout};

use crate::net::Link;
use crate::trace::{Caller, Outcome, external};

const AUTH_CODES: [&str; 4] = [
    "[AUTHENTICATIONFAILED]",
    "[AUTHORIZATIONFAILED]",
    "[EXPIRED]",
    "[PRIVACYREQUIRED]",
];
const BYE: &str = "* BYE";
const CHUNK: usize = 0x4000;
const STATUS_OK: &str = "OK";

pub(super) struct Conn {
    buffer: Vec<u8>,
    line_max: usize,
    link: Link,
    literal_max: usize,
    read: usize,
    session: String,
    tag: u32,
    user: String,
    written: usize,
}

impl Conn {
    pub(super) async fn ask(&mut self, body: &str, limit: Duration) -> Result<Reply, String> {
        let action = verb(body);
        let detail = body
            .get(action.len()..)
            .map(str::trim)
            .and_then(|rest| rest.split(" (").next())
            .filter(|_| action.starts_with("UID "))
            .unwrap_or_default();
        let mark = self.mark(action, detail);
        let result = timeout(limit, self.command(body))
            .await
            .map_err(|_elapsed| "timeout:read".to_owned())
            .and_then(|reply| reply.and_then(Reply::checked));
        self.note(
            mark,
            result
                .as_ref()
                .map_or_else(Clone::clone, |_| "success".to_owned()),
        );
        result
    }

    async fn bytes(&mut self, count: usize) -> Result<Vec<u8>, String> {
        while self.buffer.len() < count {
            self.more().await?;
        }
        Ok(self.buffer.drain(..count).collect())
    }

    fn caller(&self) -> Caller<'_> {
        Caller::new("imap", "imap", &self.user).session(&self.session)
    }

    pub(super) async fn command(&mut self, body: &str) -> Result<Reply, String> {
        let tag = self.start(body).await;
        self.finish(&tag).await
    }

    pub(super) async fn finish(&mut self, tag: &str) -> Result<Reply, String> {
        let mut untagged = Vec::new();
        let prefix = format!("{tag} ");
        loop {
            let found = self.response().await?;
            if let Some(rest) = found.text.strip_prefix(&prefix) {
                return Ok(Reply {
                    text: rest.to_owned(),
                    untagged,
                });
            }
            if found.text.starts_with(BYE) {
                return Err("imap:bye".to_owned());
            }
            untagged.push(found);
        }
    }

    async fn line(&mut self) -> Result<Vec<u8>, String> {
        loop {
            if let Some(end) = self.buffer.windows(2).position(|pair| pair == b"\r\n") {
                let mut found: Vec<u8> = self.buffer.drain(..end.saturating_add(2)).collect();
                found.truncate(end);
                return Ok(found);
            }
            if self.buffer.len() > self.line_max {
                return Err("imap:line".to_owned());
            }
            self.more().await?;
        }
    }

    pub(super) fn mark<'mark>(&self, action: &'mark str, detail: &'mark str) -> Mark<'mark> {
        Mark {
            action,
            detail,
            read: self.read,
            started: Instant::now(),
            written: self.written,
        }
    }

    async fn more(&mut self) -> Result<(), String> {
        let mut chunk = vec![u8::default(); CHUNK];
        let size = self.link.read(&mut chunk).await.unwrap_or_default();
        if size == 0 {
            return Err("imap:eof".to_owned());
        }
        chunk.truncate(size);
        self.read = self.read.saturating_add(size);
        self.buffer.extend_from_slice(&chunk);
        Ok(())
    }

    pub(super) fn named(self, user: &str, session: String) -> Self {
        Self {
            session,
            user: user.to_owned(),
            ..self
        }
    }

    pub(super) const fn new(link: Link, line_max: usize, literal_max: usize) -> Self {
        Self {
            buffer: Vec::new(),
            line_max,
            link,
            literal_max,
            read: 0,
            session: String::new(),
            tag: 0,
            user: String::new(),
            written: 0,
        }
    }

    pub(super) fn note(&self, mark: Mark<'_>, status: String) {
        let elapsed = u64::try_from(mark.started.elapsed().as_micros()).unwrap_or(u64::MAX);
        let outcome = Outcome::new(self.read.saturating_sub(mark.read), status, elapsed)
            .within(self.caller())
            .params(mark.detail.to_owned());
        external(
            mark.action,
            self.written.saturating_sub(mark.written),
            &outcome,
        );
    }

    pub(super) async fn response(&mut self) -> Result<Response, String> {
        let mut text = String::new();
        let mut literals = Vec::new();
        loop {
            let line = self.line().await?;
            let piece = String::from_utf8_lossy(&line);
            text.push_str(&piece);
            let Some(size) = literal(&piece) else {
                return Ok(Response { literals, text });
            };
            if size > self.literal_max {
                return Err("imap:literal".to_owned());
            }
            literals.push(self.bytes(size).await?);
        }
    }

    pub(super) async fn send(&mut self, line: &str) {
        let bytes = format!("{line}\r\n");
        self.written = self.written.saturating_add(bytes.len());
        drop(self.link.write_all(bytes.as_bytes()).await);
    }

    pub(super) fn session(&self) -> &str {
        &self.session
    }

    pub(super) async fn start(&mut self, body: &str) -> String {
        self.tag = self.tag.saturating_add(1);
        let tag = format!("A{}", self.tag);
        self.send(&format!("{tag} {body}")).await;
        tag
    }
}

#[derive(Clone, Copy)]
pub(super) struct Mark<'mark> {
    action: &'mark str,
    detail: &'mark str,
    read: usize,
    started: Instant,
    written: usize,
}

pub(super) struct Reply {
    text: String,
    untagged: Vec<Response>,
}

impl Reply {
    pub(super) fn checked(self) -> Result<Self, String> {
        let status = self.text.split_whitespace().next().unwrap_or_default();
        if status.eq_ignore_ascii_case(STATUS_OK) {
            Ok(self)
        } else if AUTH_CODES.iter().any(|code| self.text.contains(code)) {
            Err("imap:auth".to_owned())
        } else {
            Err(format!("imap:{}", status.to_lowercase()))
        }
    }

    pub(super) fn untagged(&self) -> &[Response] {
        &self.untagged
    }
}

pub(super) struct Response {
    literals: Vec<Vec<u8>>,
    text: String,
}

impl Response {
    pub(super) fn literals(&self) -> &[Vec<u8>] {
        &self.literals
    }

    pub(super) fn text(&self) -> &str {
        &self.text
    }
}

fn literal(piece: &str) -> Option<usize> {
    let inner = piece.strip_suffix('}')?;
    let (_, size) = inner.rsplit_once('{')?;
    size.trim_end_matches('+').parse::<usize>().ok()
}

pub(super) fn quoted(text: &str) -> Result<String, String> {
    if text
        .chars()
        .any(|letter| !letter.is_ascii() || letter.is_ascii_control())
    {
        return Err("imap:credential".to_owned());
    }
    Ok(format!(
        "\"{}\"",
        text.replace('\\', "\\\\").replace('"', "\\\"")
    ))
}

fn verb(body: &str) -> &str {
    body.match_indices(' ')
        .nth(usize::from(body.starts_with("UID ")))
        .map_or(body, |(index, _)| body.get(..index).unwrap_or(body))
}
