use core::str::from_utf8;
use core::time::Duration;
use std::io::{self, ErrorKind, Read, Write};
use std::net::TcpStream as StdStream;

use http::Uri;
use security_framework::certificate::SecCertificate;
use security_framework::secure_transport::{
    ClientBuilder, ClientHandshakeError, MidHandshakeClientBuilder, SslStream,
};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout};

const ATTEMPTS: u32 = 2;

#[derive(Clone, Copy, Debug)]
pub struct Call<'call> {
    body: &'call str,
    key: Option<&'call str>,
    timeout_ms: u64,
    url: &'call Uri,
}

impl<'call> Call<'call> {
    #[must_use = "调用需交给 Client"]
    pub const fn new(url: &'call Uri, body: &'call str, timeout_ms: u64) -> Self {
        Self {
            body,
            key: None,
            timeout_ms,
            url,
        }
    }

    #[must_use = "调用需交给 Client"]
    pub const fn with_key(self, key: Option<&'call str>) -> Self {
        Self { key, ..self }
    }
}

struct Channel {
    plain: Option<TcpStream>,
    tls: Option<SslStream<Socket>>,
}

impl Channel {
    const fn plain(stream: TcpStream) -> Self {
        Self {
            plain: Some(stream),
            tls: None,
        }
    }

    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, String> {
        if let Some(stream) = self.plain.as_mut() {
            stream.read(buffer).await.map_err(|_err| "io".to_owned())
        } else {
            read_tls(self.tls.as_mut().ok_or_else(|| "io".to_owned())?, buffer).await
        }
    }

    const fn tls(stream: SslStream<Socket>) -> Self {
        Self {
            plain: None,
            tls: Some(stream),
        }
    }

    async fn write_all(&mut self, bytes: &[u8]) -> Result<(), String> {
        if let Some(stream) = self.plain.as_mut() {
            stream
                .write_all(bytes)
                .await
                .map_err(|_err| "io".to_owned())
        } else {
            write_tls(self.tls.as_mut().ok_or_else(|| "io".to_owned())?, bytes).await
        }
    }
}

#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct Client {
    anchors: Vec<SecCertificate>,
}

impl Client {
    async fn connect(&self, url: &Uri) -> Result<Channel, String> {
        let host = url.host().ok_or_else(|| "connect".to_owned())?;
        let port = url.port().map_or_else(
            || {
                if url.scheme_str() == Some("https") {
                    443
                } else {
                    80
                }
            },
            |port| port.as_u16(),
        );
        let tcp = TcpStream::connect((host, port))
            .await
            .map_err(|_err| "connect".to_owned())?;
        if url.scheme_str() == Some("https") {
            let mut builder = ClientBuilder::new();
            if !self.anchors.is_empty() {
                let _insecure = builder.danger_accept_invalid_certs(true);
            }
            let socket = Socket::new(tcp)?;
            let _debug = std::fs::write(
                std::env::temp_dir().join("client-builder.txt"),
                format!("{builder:?}"),
            );
            let tls = match builder.handshake(host, socket) {
                Ok(tls) => tls,
                Err(ClientHandshakeError::Interrupted(tls)) => handshake(tls).await?,
                Err(ClientHandshakeError::Failure(_err)) => return Err("tls".to_owned()),
            };
            let mut stream = tls;
            stream.get_mut().set_nonblocking()?;
            Ok(Channel::tls(stream))
        } else {
            Ok(Channel::plain(tcp))
        }
    }

    async fn exchange(&self, call: &Call<'_>) -> Result<Reply, String> {
        let mut stream = self.connect(call.url).await?;
        stream.write_all(request(call).as_bytes()).await?;
        let mut bytes = Vec::new();
        loop {
            let mut buffer = vec![u8::default(); 0x2000];
            let size = stream.read(&mut buffer).await?;
            if size == 0 {
                return reply(&bytes);
            }
            bytes.extend_from_slice(buffer.get(..size).ok_or_else(|| "io".to_owned())?);
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
        timeout(Duration::from_millis(call.timeout_ms), self.exchange(call))
            .await
            .map_err(|_err| "timeout".to_owned())?
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

struct Socket(StdStream);

impl Socket {
    fn new(stream: TcpStream) -> Result<Self, String> {
        Ok(Self(stream.into_std().map_err(|_err| "tls".to_owned())?))
    }

    async fn ready(&self) {
        sleep(Duration::from_millis(1)).await;
    }

    fn set_nonblocking(&self) -> Result<(), String> {
        self.0
            .set_nonblocking(true)
            .map_err(|_err| "tls".to_owned())
    }
}

impl Read for Socket {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl Write for Socket {
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }

    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
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
        let end = size.saturating_add(2);
        if rest.len() < end || rest.get(size..end) != Some(b"\r\n") {
            return Err("io".to_owned());
        }
        output.extend_from_slice(rest.get(..size).ok_or_else(|| "io".to_owned())?);
        bytes = rest.get(end..).ok_or_else(|| "io".to_owned())?;
    }
}

fn complete(bytes: &[u8]) -> Option<usize> {
    let (raw, body) = split(bytes, b"\r\n\r\n")?;
    let head = parse_head(raw).ok()?;
    let start = bytes.len().saturating_sub(body.len());
    match head.framing {
        Framing::Chunked => chunked(body).ok().map(|_| bytes.len()),
        Framing::Close => Some(bytes.len()),
        Framing::Length(size) => {
            let end = start.saturating_add(size);
            (body.len() >= size).then_some(end)
        }
    }
}

async fn handshake(
    mut stream: MidHandshakeClientBuilder<Socket>,
) -> Result<SslStream<Socket>, String> {
    for _ in 0..ATTEMPTS {
        match stream.handshake() {
            Ok(done) => return Ok(done),
            Err(ClientHandshakeError::Interrupted(next)) => {
                stream = next;
                stream.get_mut().ready().await;
            }
            Err(ClientHandshakeError::Failure(_err)) => return Err("tls".to_owned()),
        }
    }
    Err("tls".to_owned())
}

fn line(bytes: &[u8]) -> Option<Parts<'_>> {
    split(bytes, b"\r\n")
}

fn parse_head(bytes: &[u8]) -> Result<Head, String> {
    let text = String::from_utf8_lossy(bytes);
    let mut lines = text.split("\r\n");
    let status = lines
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| "io".to_owned())?;
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

async fn read_tls(stream: &mut SslStream<Socket>, buffer: &mut [u8]) -> Result<usize, String> {
    loop {
        match stream.read(buffer) {
            Ok(size) => return Ok(size),
            Err(err) if err.kind() == ErrorKind::WouldBlock => {
                stream.get_mut().ready().await;
            }
            Err(_err) => return Err("io".to_owned()),
        }
    }
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
    Some((
        bytes.get(..position)?,
        bytes.get(position.saturating_add(needle.len())..)?,
    ))
}

async fn write_tls(stream: &mut SslStream<Socket>, mut bytes: &[u8]) -> Result<(), String> {
    while !bytes.is_empty() {
        match stream.write(bytes) {
            Ok(0) => return Err("io".to_owned()),
            Ok(size) => {
                bytes = bytes.get(size..).ok_or_else(|| "io".to_owned())?;
            }
            Err(err) if err.kind() == ErrorKind::WouldBlock => {
                stream.get_mut().ready().await;
            }
            Err(_err) => return Err("io".to_owned()),
        }
    }
    Ok(())
}
