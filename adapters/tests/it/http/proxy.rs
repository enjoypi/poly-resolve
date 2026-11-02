use std::io::{self, Read as _, Write as _};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::mpsc::{Receiver, channel};
use std::thread::{JoinHandle, sleep, spawn};

use adapters::http::{Call, Client, Reply};
use http::Uri;
use security_framework::certificate::SecCertificate;

use super::{
    BODY, CLIENT_TIMEOUT_MS, FLOOD, LAG, RECV_TIMEOUT, STATUS_OK_BODY, TIMEOUT_MS, serve_tls,
};

const ESTABLISHED: &str = "HTTP/1.1 200 Connection established\r\n\r\n";
const OVERSIZED: usize = 0x2400;

type Answer = fn(TcpStream, &str) -> io::Result<()>;
type Held = (Uri, JoinHandle<()>);

struct Relay {
    heads: Receiver<String>,
    url: Uri,
    worker: JoinHandle<()>,
}

fn connect_target(head: &str) -> String {
    head.split_whitespace()
        .nth(1)
        .unwrap_or_default()
        .to_owned()
}

fn hold(scheme: &str) -> io::Result<Held> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let url = format!("{scheme}://localhost:{port}/v1")
        .parse::<Uri>()
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let worker = spawn(move || {
        for stream in listener.incoming().take(1).flatten() {
            sleep(LAG);
            drop(stream);
        }
    });
    Ok((url, worker))
}

#[tokio::test]
async fn https_tunnels_through_the_proxy() {
    let target = serve_tls().unwrap();
    let relay = relay(|mut client, head| {
        client.write_all(ESTABLISHED.as_bytes())?;
        pipe(client, &connect_target(head))
    })
    .unwrap();
    let cert = SecCertificate::from_der(include_bytes!("../fixtures/tls/cert.der")).unwrap();
    let call = Call::new(&target.url, BODY, TIMEOUT_MS)
        .with_key(Some("k"))
        .with_proxy(Some(&relay.url));
    let reply = Client::with_anchor(cert).post(&call).await.unwrap();
    assert_eq!(reply.status(), STATUS_OK_BODY);
    let head = relay.heads.recv_timeout(RECV_TIMEOUT).unwrap();
    let authority = target.url.authority().map(ToString::to_string).unwrap();
    assert_eq!(
        head,
        format!("CONNECT {authority} HTTP/1.1\r\nhost: {authority}\r\n\r\n")
    );
    target.finish();
    drop(relay.worker.join());
}

pub(crate) fn pipe(client: TcpStream, target: &str) -> io::Result<()> {
    let upstream = TcpStream::connect(target)?;
    let mut back_from = upstream.try_clone()?;
    let mut back_to = client.try_clone()?;
    let back = spawn(move || {
        let copied = io::copy(&mut back_from, &mut back_to);
        drop(back_to.shutdown(Shutdown::Both));
        copied
    });
    let mut forth_from = client;
    let mut forth_to = upstream;
    let forth = io::copy(&mut forth_from, &mut forth_to);
    drop(back.join());
    forth.map(drop)
}

async fn post_via(proxy: &Uri, timeout_ms: u64) -> Result<Reply, String> {
    let target = "https://localhost:1/v1".parse::<Uri>().unwrap_or_default();
    let call = Call::new(&target, BODY, timeout_ms).with_proxy(Some(proxy));
    Client::new().post(&call).await
}

#[tokio::test]
async fn proxy_failures_never_fall_back_to_direct() {
    let cases: [(&str, Answer); 5] = [
        ("proxy:407", |mut client, _| {
            client.write_all(b"HTTP/1.1 407 Proxy Authentication Required\r\n\r\n")
        }),
        ("proxy:502", |mut client, _| {
            client.write_all(b"HTTP/1.1 502 Bad Gateway\r\n\r\n")
        }),
        ("io", |mut client, _| client.write_all(b"junk\r\n\r\n")),
        ("io", |client, _| client.shutdown(Shutdown::Write)),
        ("io", |mut client, _| {
            client.write_all("x".repeat(OVERSIZED).as_bytes())
        }),
    ];
    for (expected, answer) in cases {
        let relay = relay(answer).unwrap();
        assert_eq!(
            post_via(&relay.url, TIMEOUT_MS).await.unwrap_err(),
            expected
        );
        drop(relay.worker.join());
    }
}

#[tokio::test]
async fn proxy_reset_maps_to_io() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let worker = spawn(move || {
        for mut stream in listener.incoming().take(1).flatten() {
            let mut first = [u8::default(); 1];
            let _read = stream.read(&mut first);
            sleep(LAG.checked_div(20).unwrap_or(LAG));
            drop(stream);
        }
    });
    let (_sender, heads) = channel();
    let url = format!("http://127.0.0.1:{port}").parse::<Uri>().unwrap();
    let relay = Relay { heads, url, worker };
    assert_eq!(post_via(&relay.url, TIMEOUT_MS).await.unwrap_err(), "io");
    drop(relay.worker.join());
}

fn read_head(stream: &mut TcpStream) -> io::Result<String> {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        let mut byte = [u8::default(); 1];
        if stream.read(&mut byte)? == 0 {
            break;
        }
        bytes.extend_from_slice(&byte);
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn relay(answer: Answer) -> io::Result<Relay> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let url = format!("http://127.0.0.1:{port}")
        .parse::<Uri>()
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let (sender, heads) = channel();
    let worker = spawn(move || {
        for mut stream in listener.incoming().take(1).flatten() {
            let head = read_head(&mut stream).unwrap_or_default();
            drop(sender.send(head.clone()));
            drop(answer(stream, &head));
        }
    });
    Ok(Relay { heads, url, worker })
}

#[tokio::test]
async fn silent_stages_report_where_time_ran_out() {
    let relay = relay(|_client, _| {
        sleep(LAG);
        Ok(())
    })
    .unwrap();
    assert_eq!(
        post_via(&relay.url, CLIENT_TIMEOUT_MS).await.unwrap_err(),
        "timeout:proxy"
    );
    drop(relay.worker.join());
    let quiet = self::relay(|_client, _| {
        sleep(LAG);
        Ok(())
    })
    .unwrap();
    let target = "https://localhost:1/v1".parse::<Uri>().unwrap();
    let knock = Call::new(&target, "", CLIENT_TIMEOUT_MS).with_proxy(Some(&quiet.url));
    assert_eq!(
        Client::new().probe(&knock).await,
        Err("timeout:proxy".to_owned())
    );
    drop(quiet.worker.join());
    let (tls, tls_worker) = hold("https").unwrap();
    let handshake = Client::new()
        .post(&Call::new(&tls, BODY, CLIENT_TIMEOUT_MS))
        .await
        .unwrap_err();
    assert_eq!(handshake, "timeout:tls");
    drop(tls_worker.join());
    let (plain, plain_worker) = hold("http").unwrap();
    let flood = "x".repeat(FLOOD);
    let sending = Client::new()
        .post(&Call::new(&plain, &flood, CLIENT_TIMEOUT_MS))
        .await
        .unwrap_err();
    assert_eq!(sending, "timeout:send");
    drop(plain_worker.join());
}

#[tokio::test]
async fn unreachable_proxy_maps_to_proxy_connect() {
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let (_sender, heads) = channel();
    let url = format!("http://127.0.0.1:{port}").parse::<Uri>().unwrap();
    let relay = Relay {
        heads,
        url,
        worker: spawn(|| {}),
    };
    assert_eq!(
        post_via(&relay.url, TIMEOUT_MS).await.unwrap_err(),
        "proxy_connect"
    );
    drop(relay.worker.join());
}
