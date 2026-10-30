use core::time::Duration;
use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpListener};
use std::process::Command;
use std::sync::mpsc::{Receiver, channel};
use std::thread::{JoinHandle, sleep, spawn};

use adapters::http::{Call, Client};
use http::Uri;
use security_framework::certificate::SecCertificate;

const BODY: &str = r#"{"input":"hello"}"#;
const CLIENT_TIMEOUT_MS: u64 = 100;
const LAG: Duration = Duration::from_secs(1);
const OK_BODY: &str =
    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\n\r\n{\"ok\":true}";
const PATH: &str = "/v1/decide";
const RECV_TIMEOUT: Duration = Duration::from_secs(4);
const STATUS_OK_BODY: u16 = 200;
const TIMEOUT_MS: u64 = 2_000;

#[derive(Debug, Default)]
struct Seen {
    auth: Option<String>,
    body: String,
    content_type: Option<String>,
    method: String,
    path: String,
}

#[derive(Debug)]
struct Stub {
    pid: Option<u32>,
    seen: Receiver<Seen>,
    url: Uri,
    worker: JoinHandle<()>,
}

impl Stub {
    fn finish(self) {
        if let Some(pid) = self.pid {
            let _killed = Command::new("kill").arg(pid.to_string()).status();
        }
        drop(self.worker.join());
    }
}

#[tokio::test]
async fn chunked_and_close_delimited_responses_decode() {
    let chunked = serve(|_| {
        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\n{\"ok\"\r\n6\r\n:true}\r\n0\r\n\r\n"
            .to_owned()
    })
    .unwrap();
    let reply = Client::new()
        .post(&Call::new(&chunked.url, BODY, TIMEOUT_MS))
        .await
        .unwrap();
    assert_eq!(reply.body(), r#"{"ok":true}"#);
    chunked.finish();
    let close = serve(|_| "HTTP/1.1 200 OK\r\n\r\n{\"ok\":true}".to_owned()).unwrap();
    let close_reply = Client::new()
        .post(&Call::new(&close.url, BODY, TIMEOUT_MS))
        .await
        .unwrap();
    assert_eq!(close_reply.body(), r#"{"ok":true}"#);
    close.finish();
}

#[tokio::test]
async fn chunked_rejects_bad_terminator_and_size() {
    let truncated = serve(|_| {
        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\n{\"ok\"\r\nX".to_owned()
    })
    .unwrap();
    let error = Client::new()
        .post(&Call::new(&truncated.url, BODY, TIMEOUT_MS))
        .await
        .unwrap_err();
    assert_eq!(error, "io");
    truncated.finish();
    let invalid =
        serve(|_| "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nzz\r\n".to_owned())
            .unwrap();
    let invalid_error = Client::new()
        .post(&Call::new(&invalid.url, BODY, TIMEOUT_MS))
        .await
        .unwrap_err();
    assert_eq!(invalid_error, "io");
    invalid.finish();
}

#[tokio::test]
async fn connect_refused_maps_to_connect() {
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let url = format!("http://127.0.0.1:{port}{PATH}")
        .parse::<Uri>()
        .unwrap();
    let error = Client::new()
        .post(&Call::new(&url, BODY, TIMEOUT_MS))
        .await
        .unwrap_err();
    assert_eq!(error, "connect");
}

fn content_length(head: &str) -> usize {
    header(head, "content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_default()
}

#[tokio::test]
async fn garbage_response_maps_to_io() {
    let stub = serve(|_| "junk\r\n\r\n".to_owned()).unwrap();
    let error = Client::new()
        .post(&Call::new(&stub.url, BODY, TIMEOUT_MS))
        .await
        .unwrap_err();
    assert_eq!(error, "io");
    let seen = stub.seen.recv_timeout(RECV_TIMEOUT).unwrap();
    assert_eq!(seen.body, BODY);
    stub.finish();
}

fn header(head: &str, name: &str) -> Option<String> {
    head.split("\r\n")
        .filter_map(|line| line.split_once(": "))
        .find(|entry| entry.0.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.to_owned())
}

#[tokio::test]
async fn https_posts_json_with_trusted_anchor() {
    let stub = serve_tls().unwrap();
    let cert = SecCertificate::from_der(include_bytes!("fixtures/tls/cert.der")).unwrap();
    let reply = Client::with_anchor(cert)
        .post(&Call::new(&stub.url, BODY, TIMEOUT_MS))
        .await
        .unwrap();
    assert_eq!(reply.status(), STATUS_OK_BODY);
    stub.finish();
}

#[tokio::test]
async fn https_rejects_untrusted_certificate() {
    let stub = serve_tls().unwrap();
    let error = Client::new()
        .post(&Call::new(&stub.url, BODY, TIMEOUT_MS))
        .await
        .unwrap_err();
    assert_eq!(error, "tls");
    stub.finish();
}

#[tokio::test]
async fn non_2xx_maps_to_http_status() {
    let stub = serve(|_| "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_owned()).unwrap();
    let error = Client::default()
        .post(&Call::new(&stub.url, BODY, TIMEOUT_MS))
        .await
        .unwrap_err();
    assert_eq!(error, "http:404");
    assert!(!error.contains("127.0.0.1"));
    stub.finish();
}

#[tokio::test]
async fn posts_json_with_bearer_key() {
    let stub = serve(|_| OK_BODY.to_owned()).unwrap();
    let call = Call::new(&stub.url, BODY, TIMEOUT_MS).with_key(Some("k"));
    let reply = Client::new().post(&call).await.unwrap();
    assert_eq!(reply.status(), STATUS_OK_BODY);
    assert_eq!(reply.body(), r#"{"ok":true}"#);
    let seen = stub.seen.recv_timeout(RECV_TIMEOUT).unwrap();
    assert_eq!(seen.method, "POST");
    assert_eq!(seen.path, PATH);
    assert_eq!(seen.content_type.as_deref(), Some("application/json"));
    assert_eq!(seen.auth.as_deref(), Some("Bearer k"));
    assert_eq!(seen.body, BODY);
    stub.finish();
}

fn read_request<S>(stream: &mut S) -> io::Result<Seen>
where
    S: Read + Write,
{
    let mut bytes = vec![u8::default(); 0x2000];
    let count = stream.read(&mut bytes)?;
    if count == 0 {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "request"));
    }
    bytes.truncate(count);
    let text = String::from_utf8_lossy(&bytes);
    let (head, rest) = text
        .split_once("\r\n\r\n")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "request"))?;
    let length = content_length(head);
    let body = rest.get(..length).unwrap_or(rest).to_owned();
    let request_line = head.split("\r\n").next().unwrap_or_default();
    let mut parts = request_line.split(' ');
    let method = parts.next().unwrap_or_default().to_owned();
    let path = parts.next().unwrap_or_default().to_owned();
    Ok(Seen {
        auth: header(head, "authorization"),
        body,
        content_type: header(head, "content-type"),
        method,
        path,
    })
}

#[tokio::test]
async fn redirect_is_not_followed() {
    let stub = serve(|_| {
        "HTTP/1.1 301 Moved Permanently\r\nLocation: /elsewhere\r\nContent-Length: 0\r\n\r\n"
            .to_owned()
    })
    .unwrap();
    let error = Client::new()
        .post(&Call::new(&stub.url, BODY, TIMEOUT_MS))
        .await
        .unwrap_err();
    assert_eq!(error, "http:301");
    let seen = stub.seen.recv_timeout(RECV_TIMEOUT).unwrap();
    assert_eq!(seen.path, PATH);
    let _timeout = stub
        .seen
        .recv_timeout(Duration::from_millis(200))
        .unwrap_err();
    stub.finish();
}

#[tokio::test]
async fn request_without_key_omits_authorization() {
    let stub = serve(|_| OK_BODY.to_owned()).unwrap();
    let reply = Client::new()
        .post(&Call::new(&stub.url, BODY, TIMEOUT_MS))
        .await
        .unwrap();
    assert_eq!(reply.status(), STATUS_OK_BODY);
    let seen = stub.seen.recv_timeout(RECV_TIMEOUT).unwrap();
    assert!(seen.auth.is_none());
    assert_eq!(seen.body, BODY);
    stub.finish();
}

fn serve(answer: impl Fn(&Seen) -> String + Send + 'static) -> io::Result<Stub> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    let url = format!("http://{address}{PATH}")
        .parse::<Uri>()
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let (sender, seen) = channel();
    let worker = spawn(move || {
        for connection in listener.incoming().take(1).flatten() {
            let mut stream = connection;
            match read_request(&mut stream) {
                Ok(request) => {
                    let response = answer(&request);
                    drop(sender.send(request));
                    let _written = stream.write_all(response.as_bytes());
                    let _shutdown = stream.shutdown(Shutdown::Write);
                }
                Err(err) => {
                    let request = Seen {
                        body: err.to_string(),
                        ..Seen::default()
                    };
                    let response = answer(&request);
                    drop(sender.send(request));
                    let _written = stream.write_all(response.as_bytes());
                    let _shutdown = stream.shutdown(Shutdown::Write);
                }
            }
        }
    });
    Ok(Stub {
        pid: None,
        seen,
        url,
        worker,
    })
}

fn serve_tls() -> io::Result<Stub> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    drop(listener);
    let url = format!("https://localhost:{}{PATH}", address.port())
        .parse::<Uri>()
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let mut child = Command::new("openssl")
        .args([
            "s_server",
            "-accept",
            &address.port().to_string(),
            "-cert",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/it/fixtures/tls/cert.der"
            ),
            "-certform",
            "DER",
            "-key",
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/it/fixtures/tls/key.pem"),
            "-www",
            "-naccept",
            "1",
            "-tls1_2",
        ])
        .spawn()?;
    let pid = child.id();
    let worker = spawn(move || {
        let _status = child.wait();
    });
    let (_sender, seen) = channel();
    sleep(Duration::from_millis(50));
    Ok(Stub {
        pid: Some(pid),
        seen,
        url,
        worker,
    })
}

#[tokio::test]
async fn timeout_maps_to_timeout() {
    let stub = serve(|_| {
        sleep(LAG);
        OK_BODY.to_owned()
    })
    .unwrap();
    let error = Client::new()
        .post(&Call::new(&stub.url, BODY, CLIENT_TIMEOUT_MS))
        .await
        .unwrap_err();
    assert_eq!(error, "timeout");
    stub.finish();
}

#[tokio::test]
async fn truncated_response_maps_to_io() {
    let stub =
        serve(|_| "HTTP/1.1 200 OK\r\nContent-Length: 40\r\n\r\n{\"ok\":tr".to_owned()).unwrap();
    let error = Client::new()
        .post(&Call::new(&stub.url, BODY, TIMEOUT_MS))
        .await
        .unwrap_err();
    assert_eq!(error, "io");
    let seen = stub.seen.recv_timeout(RECV_TIMEOUT).unwrap();
    assert_eq!(seen.body, BODY);
    stub.finish();
}
