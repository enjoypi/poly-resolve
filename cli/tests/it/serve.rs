use std::io::{self, BufRead as _, BufReader, Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, ExitCode, Stdio};

use serde_json::{Value, from_str};

use crate::support::{Stderr, YAML, await_log, config, run, terminate};

const IDENTITY: &str = "x-session-id: s1\r\nx-user: u1\r\n";
const JSON: &str = "\r\ncontent-type: application/json\r\n";

struct Server {
    addr: String,
    child: Child,
    lines: Stderr,
}

#[tokio::test]
async fn bind_failure_uses_configured_exit_code() {
    let taken = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = taken.local_addr().unwrap();
    let path = config(
        "taken.yaml",
        &YAML.replace("127.0.0.1:0", &addr.to_string()),
    )
    .unwrap();
    let code = run(&["cli", "serve", "--config", &path]).await;
    assert_eq!(code, ExitCode::from(9));
}

#[test]
fn drains_in_flight_request() {
    let path = config("drain.yaml", YAML).unwrap();
    let mut server = start(&path).unwrap();
    let mut slow = partial(&server.addr).unwrap();
    let sync = post(&server.addr, ["/health/check", IDENTITY], "{}").unwrap();
    assert!(sync.starts_with("HTTP/1.1 200"));
    terminate(&server.child, "-TERM").unwrap();
    await_log(&mut server.lines, "shutdown signal received").unwrap();
    slow.write_all(b"}").unwrap();
    let mut response = String::new();
    let _: usize = slow.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"));
    assert!(server.child.wait().unwrap().success());
    let rest: Vec<String> = server.lines.map(Result::unwrap).collect();
    let [_, complete] = <[String; 2]>::try_from(rest).unwrap();
    assert!(complete.contains("shutdown complete"));
}

fn partial(addr: &str) -> io::Result<TcpStream> {
    let mut stream = TcpStream::connect(addr)?;
    write!(
        stream,
        "POST /health/check HTTP/1.1\r\nHost: x\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{"
    )?;
    Ok(stream)
}

fn post(addr: &str, head: [&str; 2], body: &str) -> io::Result<String> {
    let [path, headers] = head;
    let mut stream = TcpStream::connect(addr)?;
    write!(
        stream,
        "POST {path} HTTP/1.1\r\nHost: x\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )?;
    let mut response = String::new();
    let _: usize = stream.read_to_string(&mut response)?;
    Ok(response)
}

#[test]
fn serves_http_until_signal() {
    let path = config("serve.yaml", YAML).unwrap();
    for signal in ["-INT", "-TERM"] {
        let mut server = start(&path).unwrap();
        let addr = &server.addr;
        let responses = [
            (
                "HTTP/1.1 200",
                post(addr, ["/health/check", IDENTITY], "{}").unwrap(),
            ),
            (
                "HTTP/1.1 400",
                post(addr, ["/health/check", IDENTITY], "nope").unwrap(),
            ),
            (
                "HTTP/1.1 404",
                post(addr, ["/nope/nope", ""], "{}").unwrap(),
            ),
        ];
        let logged: Vec<String> = server.lines.by_ref().take(3).map(Result::unwrap).collect();
        terminate(&server.child, signal).unwrap();
        assert!(server.child.wait().unwrap().success());
        let rest: Vec<String> = server.lines.map(Result::unwrap).collect();
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
        let [received, complete] = <[String; 2]>::try_from(rest).unwrap();
        assert!(received.contains("shutdown signal received"));
        assert!(complete.contains("shutdown complete"));
    }
}

fn start(config: &str) -> io::Result<Server> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_cli"))
        .args(["serve", "--config"])
        .arg(config)
        .stderr(Stdio::piped())
        .spawn()?;
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

#[test]
fn times_out_with_configured_exit_code() {
    let yaml = YAML.replace("shutdown_timeout_ms: 10000", "shutdown_timeout_ms: 50");
    let path = config("timeout.yaml", &yaml).unwrap();
    let mut server = start(&path).unwrap();
    let slow = partial(&server.addr).unwrap();
    let sync = post(&server.addr, ["/health/check", IDENTITY], "{}").unwrap();
    assert!(sync.starts_with("HTTP/1.1 200"));
    terminate(&server.child, "-TERM").unwrap();
    let expected: i32 = 8;
    assert_eq!(server.child.wait().unwrap().code(), Some(expected));
    drop(slow);
    let rest: Vec<String> = server.lines.map(Result::unwrap).collect();
    let [_, received, timeout] = <[String; 3]>::try_from(rest).unwrap();
    assert!(received.contains("shutdown signal received"));
    assert!(timeout.contains("shutdown timeout"));
    assert!(timeout.contains(r#""exit_code":8"#));
}
