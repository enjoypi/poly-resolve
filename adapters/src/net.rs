mod tunnel;

use core::net::IpAddr;
use std::io::{self, ErrorKind, Read, Write};

use http::Uri;
use security_framework::certificate::SecCertificate;
use security_framework::secure_transport::{ClientBuilder, ClientHandshakeError, SslStream};
use tokio::io::Interest;
use tokio::net::TcpStream;

use self::tunnel::{connect, socks};

const HTTP: &str = "http";
const PROXY_SCHEMES: [&str; 3] = [HTTP, "socks5", "socks5h"];

#[derive(Clone, Copy)]
pub(crate) struct Dial<'dial> {
    anchors: &'dial [SecCertificate],
    host: &'dial str,
    layer: Layer,
    port: u16,
    proxy: Option<&'dial Uri>,
}

impl<'dial> Dial<'dial> {
    pub(crate) const fn new(host: &'dial str, port: u16, layer: Layer) -> Self {
        Self {
            anchors: &[],
            host,
            layer,
            port,
            proxy: None,
        }
    }

    pub(crate) const fn with_anchors(self, anchors: &'dial [SecCertificate]) -> Self {
        Self { anchors, ..self }
    }

    pub(crate) const fn with_proxy(self, proxy: Option<&'dial Uri>) -> Self {
        Self { proxy, ..self }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Layer {
    Plain,
    Tls,
}

pub(crate) struct Link {
    stream: Box<dyn Stream>,
}

impl Link {
    pub(crate) async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, String> {
        pull(self.stream.as_mut(), buffer).await
    }

    pub(crate) async fn write_all(&mut self, bytes: &[u8]) -> Result<(), String> {
        push(self.stream.as_mut(), bytes).await
    }
}

impl Stream for SslStream<Wire> {
    fn wire(&self) -> &Wire {
        self.get_ref()
    }
}

trait Stream: Read + Send + Write {
    fn wire(&self) -> &Wire;
}

struct Wire {
    tcp: TcpStream,
    want: Interest,
}

impl Read for Wire {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.want = Interest::READABLE;
        self.tcp.try_read(buf)
    }
}

impl Stream for Wire {
    fn wire(&self) -> &Wire {
        self
    }
}

impl Write for Wire {
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.want = Interest::WRITABLE;
        self.tcp.try_write(buf)
    }
}

pub(crate) fn code(line: &str) -> Option<u16> {
    line.split_whitespace()
        .nth(1)
        .and_then(|found| found.parse::<u16>().ok())
}

async fn handshake(dial: &Dial<'_>, wire: Wire) -> Result<Link, String> {
    let mut builder = ClientBuilder::new();
    let _anchored = builder.anchor_certificates(dial.anchors);
    let mut attempt = builder.handshake(dial.host, wire);
    loop {
        match attempt {
            Ok(tls) => {
                return Ok(Link {
                    stream: Box::new(tls),
                });
            }
            Err(ClientHandshakeError::Interrupted(pending)) => {
                wait(pending.get_ref()).await;
                attempt = pending.handshake();
            }
            Err(ClientHandshakeError::Failure(_err)) => return Err("tls".to_owned()),
        }
    }
}

pub(crate) fn loopback(url: &Uri) -> bool {
    url.host().is_some_and(|host| {
        host == "localhost"
            || host
                .trim_start_matches('[')
                .trim_end_matches(']')
                .parse::<IpAddr>()
                .is_ok_and(|address| address.is_loopback())
    })
}

pub(crate) async fn open(dial: &Dial<'_>, stage: &mut &'static str) -> Result<Link, String> {
    let wire = match dial.proxy {
        Some(proxy) => {
            *stage = "proxy";
            tunneled(dial, proxy).await?
        }
        None => plain(dial.host, dial.port)
            .await
            .map_err(|_err| "connect".to_owned())?,
    };
    if dial.layer == Layer::Tls {
        *stage = "tls";
        handshake(dial, wire).await
    } else {
        Ok(Link {
            stream: Box::new(wire),
        })
    }
}

async fn plain(host: &str, port: u16) -> io::Result<Wire> {
    Ok(Wire {
        tcp: TcpStream::connect((host, port)).await?,
        want: Interest::READABLE,
    })
}

pub(crate) fn proxied(text: &str) -> Result<Uri, String> {
    text.parse::<Uri>()
        .ok()
        .filter(|url| {
            let scheme = url.scheme_str().unwrap_or_default();
            loopback(url)
                && PROXY_SCHEMES.contains(&scheme)
                && format!(
                    "{scheme}://{}:{}",
                    url.host().unwrap_or_default(),
                    url.port_u16().unwrap_or_default()
                ) == text
        })
        .ok_or_else(|| "proxy must be http://, socks5:// or socks5h://<loopback>:<port>".to_owned())
}

async fn pull(stream: &mut dyn Stream, buffer: &mut [u8]) -> Result<usize, String> {
    loop {
        match stream.read(buffer) {
            Err(err) if err.kind() == ErrorKind::WouldBlock => wait(stream.wire()).await,
            done => return done.map_err(|_err| "io".to_owned()),
        }
    }
}

async fn push(stream: &mut dyn Stream, mut bytes: &[u8]) -> Result<(), String> {
    while !bytes.is_empty() {
        match stream.write(bytes) {
            Ok(size) => bytes = bytes.get(size..).unwrap_or_default(),
            Err(err) if err.kind() == ErrorKind::WouldBlock => wait(stream.wire()).await,
            Err(_err) => return Err("io".to_owned()),
        }
    }
    Ok(())
}

async fn tunneled(dial: &Dial<'_>, proxy: &Uri) -> Result<Wire, String> {
    let mut wire = plain(
        proxy.host().unwrap_or_default(),
        proxy.port_u16().unwrap_or_default(),
    )
    .await
    .map_err(|_err| "proxy_connect".to_owned())?;
    if proxy.scheme_str() == Some(HTTP) {
        connect(&mut wire, dial.host, dial.port).await?;
    } else {
        socks(&mut wire, dial.host, dial.port).await?;
    }
    Ok(wire)
}

async fn wait(wire: &Wire) {
    drop(wire.tcp.ready(wire.want).await);
}
