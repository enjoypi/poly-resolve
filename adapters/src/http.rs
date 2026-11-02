use core::str::from_utf8;
use core::time::Duration;

use http::Uri;
use security_framework::certificate::SecCertificate;
use tokio::time::timeout;

use crate::net::{Dial, Layer, Link, code, open};

#[derive(Clone, Copy, Debug)]
pub struct Call<'call> {
    body: &'call str,
    key: Option<&'call str>,
    proxy: Option<&'call Uri>,
    timeout_ms: u64,
    url: &'call Uri,
}

impl<'call> Call<'call> {
    #[must_use = "调用需交给 Client"]
    pub const fn new(url: &'call Uri, body: &'call str, timeout_ms: u64) -> Self {
        Self {
            body,
            key: None,
            proxy: None,
            timeout_ms,
            url,
        }
    }

    #[must_use = "调用需交给 Client"]
    pub const fn with_key(self, key: Option<&'call str>) -> Self {
        Self { key, ..self }
    }

    #[must_use = "调用需交给 Client"]
    pub const fn with_proxy(self, proxy: Option<&'call Uri>) -> Self {
        Self { proxy, ..self }
    }
}

#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct Client {
    anchors: Vec<SecCertificate>,
}

impl Client {
    async fn connect(&self, call: &Call<'_>, stage: &mut &'static str) -> Result<Link, String> {
        let url = call.url;
        let host = url.host().ok_or_else(|| "connect".to_owned())?;
        let secure = url.scheme_str() == Some("https");
        let port = url.port_u16().unwrap_or(if secure { 443 } else { 80 });
        let layer = if secure { Layer::Tls } else { Layer::Plain };
        let dial = Dial::new(host, port, layer)
            .with_anchors(&self.anchors)
            .with_proxy(call.proxy);
        open(&dial, stage).await
    }

    async fn exchange(&self, call: &Call<'_>, stage: &mut &'static str) -> Result<Reply, String> {
        let mut link = self.connect(call, stage).await?;
        *stage = "send";
        link.write_all(request(call).as_bytes()).await?;
        *stage = "read";
        let mut bytes = Vec::new();
        loop {
            let mut buffer = vec![u8::default(); 0x2000];
            let size = link.read(&mut buffer).await?;
            if size == 0 {
                return reply(&bytes);
            }
            buffer.truncate(size);
            bytes.extend_from_slice(&buffer);
            if let Some(end) = complete(&bytes) {
                bytes.truncate(end);
                return reply(&bytes);
            }
        }
    }

    #[must_use = "客户端需复用"]
    pub const fn new() -> Self {
        Self {
            anchors: Vec::new(),
        }
    }

    pub async fn post(&self, call: &Call<'_>) -> Result<Reply, String> {
        let mut stage = "connect";
        let outcome = timeout(
            Duration::from_millis(call.timeout_ms),
            self.exchange(call, &mut stage),
        )
        .await;
        outcome.unwrap_or_else(|_elapsed| Err(format!("timeout:{stage}")))
    }

    pub async fn probe(&self, call: &Call<'_>) -> Result<(), String> {
        let mut stage = "connect";
        let outcome = timeout(
            Duration::from_millis(call.timeout_ms),
            self.connect(call, &mut stage),
        )
        .await;
        outcome
            .unwrap_or_else(|_elapsed| Err(format!("timeout:{stage}")))
            .map(drop)
    }

    #[must_use = "测试根证书需显式提供"]
    pub fn with_anchor(cert: SecCertificate) -> Self {
        Self {
            anchors: vec![cert],
        }
    }
}

#[derive(Clone, Copy)]
enum Framing {
    Chunked,
    Close,
    Length(usize),
}

struct Head {
    framing: Framing,
    status: u16,
}

type Parts<'bytes> = (&'bytes [u8], &'bytes [u8]);

#[derive(Debug)]
pub struct Reply {
    body: String,
    status: u16,
}

impl Reply {
    #[must_use = "正文需交给解析"]
    pub fn body(&self) -> &str {
        &self.body
    }

    #[must_use = "状态码需交给处理"]
    pub const fn status(&self) -> u16 {
        self.status
    }
}

fn chunked(mut bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    loop {
        let (line, rest) = line(bytes).ok_or_else(|| "io".to_owned())?;
        let size = line
            .split(|byte| *byte == b';')
            .next()
            .and_then(|raw| from_utf8(raw).ok())
            .and_then(|raw| usize::from_str_radix(raw.trim(), 16).ok())
            .ok_or_else(|| "io".to_owned())?;
        if size == 0 {
            return Ok(output);
        }
        let (chunk, tail) = rest.split_at_checked(size).ok_or_else(|| "io".to_owned())?;
        bytes = tail.strip_prefix(b"\r\n").ok_or_else(|| "io".to_owned())?;
        output.extend_from_slice(chunk);
    }
}

fn complete(bytes: &[u8]) -> Option<usize> {
    let (raw, body) = split(bytes, b"\r\n\r\n")?;
    let Ok(head) = parse_head(raw) else {
        return Some(bytes.len());
    };
    let start = bytes.len().saturating_sub(body.len());
    match head.framing {
        Framing::Chunked => chunked(body).ok().map(|_| bytes.len()),
        Framing::Close => None,
        Framing::Length(size) => {
            let end = start.saturating_add(size);
            (body.len() >= size).then_some(end)
        }
    }
}

fn line(bytes: &[u8]) -> Option<Parts<'_>> {
    split(bytes, b"\r\n")
}

fn parse_head(bytes: &[u8]) -> Result<Head, String> {
    let text = String::from_utf8_lossy(bytes);
    let mut lines = text.split("\r\n");
    let status = lines.next().and_then(code).ok_or_else(|| "io".to_owned())?;
    let mut framing = Framing::Close;
    for (name, value) in lines.filter_map(|line| line.split_once(':')) {
        if name.eq_ignore_ascii_case("content-length") {
            framing = Framing::Length(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|_err| "io".to_owned())?,
            );
        }
        if name.eq_ignore_ascii_case("transfer-encoding")
            && value.trim().eq_ignore_ascii_case("chunked")
        {
            framing = Framing::Chunked;
        }
    }
    Ok(Head { framing, status })
}

fn reply(bytes: &[u8]) -> Result<Reply, String> {
    let (raw, body) = split(bytes, b"\r\n\r\n").ok_or_else(|| "io".to_owned())?;
    let head = parse_head(raw)?;
    let decoded = match head.framing {
        Framing::Chunked => chunked(body)?,
        Framing::Close => body.to_vec(),
        Framing::Length(size) => {
            if body.len() != size {
                return Err("io".to_owned());
            }
            body.to_vec()
        }
    };
    let text = String::from_utf8_lossy(&decoded).into_owned();
    if (200..300).contains(&head.status) {
        Ok(Reply {
            body: text,
            status: head.status,
        })
    } else {
        Err(format!("http:{}", head.status))
    }
}

fn request(call: &Call<'_>) -> String {
    let target = call.url.path_and_query().map_or("/", |path| path.as_str());
    let host = call.url.authority().map_or("", |host| host.as_str());
    let auth = call.key.map_or_else(String::new, |key| {
        format!("authorization: Bearer {key}\r\n")
    });
    format!(
        "POST {target} HTTP/1.1\r\nhost: {host}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n{auth}\r\n{}",
        call.body.len(),
        call.body,
    )
}

fn split<'bytes>(bytes: &'bytes [u8], needle: &[u8]) -> Option<Parts<'bytes>> {
    let position = bytes
        .windows(needle.len())
        .position(|window| window == needle)?;
    let (head, tail) = bytes.split_at_checked(position).unwrap_or_default();
    Some((head, tail.get(needle.len()..).unwrap_or_default()))
}
