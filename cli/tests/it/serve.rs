mod assets;
mod host;
mod messages;
mod reviews;

use core::ops::{Deref, DerefMut};
use std::fs;
use std::io::{self, BufRead as _, BufReader, Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, ExitCode, Stdio};

use serde_json::{Value, from_str};

use crate::support::{EVENT, Stderr, await_log, config, live, run, sandbox, terminate};

const IDENTITY: &str = "x-session-id: s1\r\nx-user: u1\r\n";
const JSON: &str = "\r\ncontent-type: application/json\r\n";

struct Guard(Child);

impl Deref for Guard {
    type Target = Child;

    fn deref(&self) -> &Child {
        &self.0
    }
}

impl DerefMut for Guard {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.0
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        drop(self.0.kill());
    }
}

struct Server {
    addr: String,
    child: Guard,
    lines: Stderr,
}

#[tokio::test]
async fn bind_failure_uses_configured_exit_code() {
    let taken = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = taken.local_addr().unwrap();
    let dir = sandbox("serve-bind").unwrap();
    let path = config(
        "taken.yaml",
        &live(&dir).replace("127.0.0.1:0", &addr.to_string()),
    )
    .unwrap();
    let code = run(&["cli", "serve", "--config", &path]).await;
    assert_eq!(code, ExitCode::from(9));
}

#[test]
fn drains_in_flight_request() {
    let dir = sandbox("serve-drain").unwrap();
    let path = config("drain.yaml", &live(&dir)).unwrap();
    let mut server = start(&path).unwrap();
    let mut slow = partial(&server.addr).unwrap();
    let sync = get(&server.addr, "/api/health").unwrap();
    assert!(sync.starts_with("HTTP/1.1 200"));
    terminate(&server.child, "-TERM").unwrap();
    await_log(&mut server.lines, "shutdown signal received").unwrap();
    slow.write_all(b"}").unwrap();
    let mut response = String::new();
    let _: usize = slow.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 "));
    assert!(server.child.wait().unwrap().success());
    let rest: Vec<String> = server.lines.map(Result::unwrap).collect();
    assert!(index_of(&rest, "shutdown complete").is_some());
}

fn get(addr: &str, path: &str) -> io::Result<String> {
    send(addr, ["GET", path, ""], "")
}

fn index_of(lines: &[String], needle: &str) -> Option<usize> {
    lines.iter().position(|line| line.contains(needle))
}

fn partial(addr: &str) -> io::Result<TcpStream> {
    let mut stream = TcpStream::connect(addr)?;
    write!(
        stream,
        "PATCH /api/reviews/zz HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{"
    )?;
    Ok(stream)
}

fn raw(addr: &str, request: &str) -> io::Result<String> {
    let mut stream = TcpStream::connect(addr)?;
    stream.write_all(request.as_bytes())?;
    let mut response = String::new();
    let _: usize = stream.read_to_string(&mut response)?;
    Ok(response)
}

fn send(addr: &str, head: [&str; 3], body: &str) -> io::Result<String> {
    let [method, path, headers] = head;
    raw(
        addr,
        &format!(
            "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ),
    )
}

#[test]
fn serves_http_until_signal() {
    let dir = sandbox("serve-signal").unwrap();
    let path = config("serve.yaml", &live(&dir)).unwrap();
    for signal in ["-INT", "-TERM"] {
        let mut server = start(&path).unwrap();
        let addr = &server.addr;
        let responses = [
            (
                "HTTP/1.1 200",
                send(addr, ["GET", "/api/health", IDENTITY], "").unwrap(),
            ),
            (
                "HTTP/1.1 400",
                send(addr, ["GET", "/api/messages?limit=x", IDENTITY], "").unwrap(),
            ),
            (
                "HTTP/1.1 400",
                send(addr, ["GET", "/api/reviews?limit=0", ""], "").unwrap(),
            ),
        ];
        terminate(&server.child, signal).unwrap();
        assert!(server.child.wait().unwrap().success());
        let logged: Vec<String> = server.lines.map(Result::unwrap).collect();
        for (status, response) in responses {
            assert!(response.starts_with(status));
            assert!(response.contains(JSON));
        }
        assert_eq!(
            logged
                .iter()
                .filter(|line| line.contains(r#""user":"u1""#))
                .count(),
            2
        );
        let received = index_of(&logged, "shutdown signal received").unwrap();
        let complete = index_of(&logged, "shutdown complete").unwrap();
        assert!(received < complete);
    }
}

#[test]
fn serves_with_triage_enabled() {
    let dir = sandbox("serve-triage").unwrap();
    fs::write(format!("{dir}/listen.txt"), format!("{EVENT}\n")).unwrap();
    let yaml = live(&dir)
        .replace("mode: disabled", "mode: enabled")
        .replace("lookback_ms: 1000\n", "lookback_ms: 1000000000000\n");
    let path = config("serve-triage.yaml", &yaml).unwrap();
    let mut server = start(&path).unwrap();
    await_log(&mut server.lines, "event stored").unwrap();
    await_log(&mut server.lines, "line opened").unwrap();
    let sync = get(&server.addr, "/api/health").unwrap();
    assert!(sync.starts_with("HTTP/1.1 200"));
    assert!(stop(server).unwrap());
}

fn start(config: &str) -> io::Result<Server> {
    let mut child = Guard(
        Command::new(env!("CARGO_BIN_EXE_cli"))
            .args(["serve", "--config"])
            .arg(config)
            .stderr(Stdio::piped())
            .spawn()?,
    );
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("stderr not piped"))?;
    let mut lines = BufReader::new(stderr).lines();
    let started = lines
        .next()
        .ok_or_else(|| io::Error::other("no listening log"))??;
    let addr = from_str::<Value>(&started)?
        .pointer("/fields/addr")
        .and_then(Value::as_str)
        .ok_or_else(|| io::Error::other("no addr in listening log"))?
        .to_owned();
    Ok(Server { addr, child, lines })
}

fn stop(mut server: Server) -> io::Result<bool> {
    terminate(&server.child, "-TERM")?;
    Ok(server.child.wait()?.success())
}

#[test]
fn stops_everything_when_a_source_needs_manual_action() {
    let dir = sandbox("serve-manual").unwrap();
    let yaml = live(&dir)
        .replace("mode: disabled", "mode: enabled")
        .replace(
            "            FAKE_DIR: ",
            "            FAKE_LISTEN: exit\n            FAKE_CODE: \"2\"\n            FAKE_DIR: ",
        );
    let path = config("serve-manual.yaml", &yaml).unwrap();
    let mut server = start(&path).unwrap();
    await_log(&mut server.lines, "dws needs manual action").unwrap();
    let status = server.child.wait().unwrap();
    let manual: i32 = 17;
    assert_eq!(status.code(), Some(manual));
}

#[test]
fn times_out_with_configured_exit_code() {
    let dir = sandbox("serve-timeout").unwrap();
    let yaml = live(&dir).replace("shutdown_timeout_ms: 10000", "shutdown_timeout_ms: 50");
    let path = config("timeout.yaml", &yaml).unwrap();
    let mut server = start(&path).unwrap();
    let slow = partial(&server.addr).unwrap();
    let sync = get(&server.addr, "/api/health").unwrap();
    assert!(sync.starts_with("HTTP/1.1 200"));
    terminate(&server.child, "-TERM").unwrap();
    let expected: i32 = 8;
    assert_eq!(server.child.wait().unwrap().code(), Some(expected));
    drop(slow);
    let rest: Vec<String> = server.lines.map(Result::unwrap).collect();
    assert!(index_of(&rest, "shutdown signal received").is_some());
    let timeout = index_of(&rest, "shutdown timeout").and_then(|index| rest.get(index));
    assert!(timeout.is_some_and(|line| line.contains(r#""exit_code":8"#)));
}

#[test]
fn waits_for_collector_after_server_stops() {
    let dir = sandbox("serve-slow-collector").unwrap();
    let yaml = live(&dir).replace(
        "            FAKE_DIR: ",
        "            FAKE_LISTEN: hang\n            FAKE_DIR: ",
    );
    let path = config("slow-collector.yaml", &yaml).unwrap();
    let server = start(&path).unwrap();
    let sync = get(&server.addr, "/api/health").unwrap();
    assert!(sync.starts_with("HTTP/1.1 200"));
    assert!(stop(server).unwrap());
}
