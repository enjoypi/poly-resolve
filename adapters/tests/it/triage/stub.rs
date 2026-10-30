use core::net::Ipv4Addr;
use core::time::Duration;
use std::fs;
use std::fs::OpenOptions;
use std::io;
use std::io::Write as _;
use std::net::TcpListener;
use std::path::{Path, PathBuf};

use serde_json::{Value, from_str, json, to_string};
use tokio::io::{AsyncBufReadExt as _, AsyncReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::net::{TcpListener as AsyncListener, TcpStream};
use tokio::time::sleep;

pub(crate) const CHAT: &str = "/chat/completions";
pub(crate) const DECISIONS: &str = "/decisions";
const DEFAULT_CONFIDENCE: &str = "0.9";
const DEFAULT_PROBABILITY: &str = "0.95";
const SLEEP_TICKS: u32 = 120;
const SLEEP_TICK_MS: u64 = 250;

struct Call<'call> {
    dir: &'call Path,
    request: &'call Request,
    role: &'call str,
    stream: &'call mut TcpStream,
}

impl Call<'_> {
    async fn answer(&self) -> (u16, String) {
        let behavior = plan(self.dir, self.role);
        match text(&behavior, "mode") {
            "fail" => return (500, String::new()),
            "junk" => return (200, "not json".to_owned()),
            "slow" => self.wait().await,
            _ => {}
        }
        match self.role {
            "decide" => (200, answers(&behavior, &self.request.body)),
            _ => (200, screened(self.dir, &self.request.body)),
        }
    }

    fn record(&self) -> io::Result<()> {
        let role = self.role;
        let file = self.dir.join(format!("{role}-calls.txt"));
        let mut calls = OpenOptions::new().append(true).create(true).open(file)?;
        let entry = json!({
            "authorization": self.request.auth,
            "content_type": self.request.kind,
            "method": self.request.method,
            "path": self.request.path,
        });
        writeln!(calls, "{entry}")?;
        fs::write(
            self.dir.join(format!("{role}-body.txt")),
            &self.request.body,
        )
    }

    async fn wait(&self) {
        for _ in 0..SLEEP_TICKS {
            sleep(Duration::from_millis(SLEEP_TICK_MS)).await;
            let mut byte = [u8::default()];
            match self.stream.peek(&mut byte).await {
                Ok(0) | Err(_) => return,
                Ok(_) => {}
            }
        }
    }
}

struct Request {
    auth: String,
    body: String,
    kind: String,
    method: String,
    path: String,
}

pub(crate) struct Stub {
    dir: PathBuf,
    listener: TcpListener,
    port: u16,
}

impl Stub {
    pub(crate) fn bind(dir: PathBuf) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        Ok(Self {
            dir,
            listener,
            port,
        })
    }

    pub(crate) async fn serve(&self) -> io::Result<()> {
        let listener = AsyncListener::from_std(self.listener.try_clone()?)?;
        accept(&self.dir, &listener).await
    }

    pub(crate) fn url(&self, path: &str) -> String {
        let port = self.port;
        format!("http://127.0.0.1:{port}{path}")
    }
}

async fn accept(dir: &Path, listener: &AsyncListener) -> io::Result<()> {
    loop {
        let (mut stream, _) = listener.accept().await?;
        drop(handle(dir, &mut stream).await);
    }
}

fn answer(behavior: &Value, question: &Value) -> Value {
    let name = text(question, "name");
    if let Some(("urgent", _)) = name.split_once('_') {
        let score = probability(behavior, name);
        json!({ "name": name, "probability": score, "type": "predicate" })
    } else {
        let index = ordinal(name).saturating_sub(1);
        let pick = listed(behavior, "topics", index).unwrap_or_else(|| "new".to_owned());
        json!({ "choice": pick, "confidence": confidence(behavior), "name": name, "type": "choice" })
    }
}

fn answers(behavior: &Value, body: &str) -> String {
    let parsed: Value = from_str(body).unwrap_or(Value::Null);
    let list: Vec<Value> = parsed
        .get("questions")
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |all| {
            all.iter()
                .map(|question| answer(behavior, question))
                .collect()
        });
    to_string(&json!({ "answers": list })).unwrap_or_default()
}

fn confidence(behavior: &Value) -> f64 {
    listed(behavior, "confidence", 0)
        .unwrap_or_else(|| DEFAULT_CONFIDENCE.to_owned())
        .parse()
        .unwrap_or_default()
}

async fn handle(dir: &Path, stream: &mut TcpStream) -> io::Result<()> {
    let Some(request) = read_request(stream).await? else {
        return Ok(());
    };
    let Some(role) = role(&request.path) else {
        return write_response(stream, 404, "").await;
    };
    let call = Call {
        dir,
        request: &request,
        role,
        stream: &mut *stream,
    };
    call.record()?;
    let (status, body) = call.answer().await;
    write_response(stream, status, &body).await
}

fn listed(behavior: &Value, key: &str, index: usize) -> Option<String> {
    text(behavior, key)
        .split_whitespace()
        .nth(index)
        .map(str::to_owned)
}

fn ordinal(name: &str) -> usize {
    name.rsplit('_')
        .next()
        .and_then(|tail| tail.parse().ok())
        .unwrap_or(1)
}

fn plan(dir: &Path, role: &str) -> Value {
    let file = dir.join(format!("{role}-stub.json"));
    fs::read_to_string(file)
        .ok()
        .and_then(|body| from_str(&body).ok())
        .unwrap_or(Value::Null)
}

fn probability(behavior: &Value, name: &str) -> f64 {
    let index = ordinal(name).saturating_sub(1);
    listed(behavior, "ps", index)
        .or_else(|| listed(behavior, "p", 0))
        .unwrap_or_else(|| DEFAULT_PROBABILITY.to_owned())
        .parse()
        .unwrap_or_default()
}

async fn read_request(stream: &mut TcpStream) -> io::Result<Option<Request>> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let _request = reader.read_line(&mut line).await?;
    if line.is_empty() {
        return Ok(None);
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_owned();
    let path = parts.next().unwrap_or_default().to_owned();
    let mut auth = String::new();
    let mut kind = String::new();
    let mut length: usize = 0;
    loop {
        line.clear();
        let _header = reader.read_line(&mut line).await?;
        let header = line.trim_end();
        if header.is_empty() {
            break;
        }
        let Some((name, value)) = header.split_once(':') else {
            continue;
        };
        match name.trim().to_ascii_lowercase().as_str() {
            "authorization" => auth = value.trim().to_owned(),
            "content-length" => length = value.trim().parse().unwrap_or_default(),
            "content-type" => kind = value.trim().to_owned(),
            _ => {}
        }
    }
    let mut bytes = vec![u8::default(); length];
    let _body = reader.read_exact(&mut bytes).await?;
    Ok(Some(Request {
        auth,
        body: String::from_utf8_lossy(&bytes).into_owned(),
        kind,
        method,
        path,
    }))
}

const fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "",
    }
}

fn role(path: &str) -> Option<&'static str> {
    if path == DECISIONS {
        Some("decide")
    } else if path == CHAT {
        Some("screen")
    } else {
        None
    }
}

fn screened(dir: &Path, body: &str) -> String {
    let content = fs::read_to_string(dir.join("screen.json"))
        .ok()
        .map_or_else(|| verdicts(body), |raw| raw.trim().to_owned());
    to_string(&json!({ "choices": [{ "message": { "content": content } }] })).unwrap_or_default()
}

fn text<'row>(row: &'row Value, key: &str) -> &'row str {
    row.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn verdicts(body: &str) -> String {
    let parsed: Value = from_str(body).unwrap_or(Value::Null);
    let content = parsed
        .get("messages")
        .and_then(|messages| messages.get(1))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let inner: Value = from_str(content).unwrap_or(Value::Null);
    let lines: Vec<Value> =
        inner
            .get("lines")
            .and_then(Value::as_array)
            .map_or_else(Vec::new, |all| {
                all.iter()
                .map(|line| {
                    json!({ "line": text(line, "line"), "risk": "low", "spans": [], "topics": [] })
                })
                .collect()
            });
    to_string(&json!({ "lines": lines })).unwrap_or_default()
}

async fn write_response(stream: &mut TcpStream, status: u16, body: &str) -> io::Result<()> {
    let why = reason(status);
    let size = body.len();
    let head = format!(
        "HTTP/1.1 {status} {why}\r\ncontent-length: {size}\r\ncontent-type: application/json\r\nconnection: close\r\n\r\n"
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(body.as_bytes()).await
}
