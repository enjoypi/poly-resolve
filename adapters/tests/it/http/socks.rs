use std::io::{self, Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{Receiver, channel};
use std::thread::{JoinHandle, sleep, spawn};

use adapters::http::{Call, Client, Reply};
use http::Uri;

use super::proxy::pipe;
use super::{BODY, LAG, OK_BODY, RECV_TIMEOUT, STATUS_OK_BODY, TIMEOUT_MS, serve};

const ACCEPTED: &[u8] = &[5, 0];
const BOUND_V4: &[u8] = &[5, 0, 0, 1, 0, 0, 0, 0, 0, 0];
const LONG_HOST: usize = 0x100;
const LOW: u16 = 0xff;
const SHIFT: u16 = 8;
const ZERO: u16 = 0;

struct Socks {
    asked: Receiver<Vec<u8>>,
    url: Uri,
    worker: JoinHandle<()>,
}

fn answer(mut client: TcpStream, greeting: &[u8], reply: &[u8]) -> io::Result<Vec<u8>> {
    let mut hello = [u8::default(); 3];
    client.read_exact(&mut hello)?;
    client.write_all(greeting)?;
    if greeting != ACCEPTED {
        return Ok(Vec::new());
    }
    let mut head = [u8::default(); 5];
    client.read_exact(&mut head)?;
    let [_, _, _, _, length] = head;
    let mut rest = vec![u8::default(); usize::from(length).saturating_add(2)];
    client.read_exact(&mut rest)?;
    client.write_all(reply)?;
    let mut asked = head.to_vec();
    asked.extend_from_slice(&rest);
    if reply.len() >= BOUND_V4.len() && reply.starts_with(ACCEPTED) {
        let split = usize::from(length);
        let host = String::from_utf8_lossy(rest.get(..split).unwrap_or_default()).into_owned();
        let port = rest
            .get(split..)
            .unwrap_or_default()
            .iter()
            .fold(ZERO, |done, byte| (done << SHIFT) | u16::from(*byte));
        let address = format!("{host}:{port}");
        drop(spawn(move || pipe(client, &address)));
    }
    Ok(asked)
}

async fn post(proxy: &Uri, target: &Uri) -> Result<Reply, String> {
    Client::new()
        .post(&Call::new(target, BODY, TIMEOUT_MS).with_proxy(Some(proxy)))
        .await
}

async fn refused(greeting: &'static [u8], reply: &'static [u8]) -> String {
    let target = "http://127.0.0.1:1/v1".parse::<Uri>().unwrap_or_default();
    let Ok(proxy) = socks(greeting, reply) else {
        return "listen".to_owned();
    };
    let outcome = post(&proxy.url, &target).await;
    drop(proxy.worker.join());
    outcome.err().unwrap_or_default()
}

fn socks(greeting: &'static [u8], reply: &'static [u8]) -> io::Result<Socks> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let url = format!("socks5://127.0.0.1:{port}")
        .parse::<Uri>()
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let (sender, asked) = channel();
    let worker = spawn(move || {
        for client in listener.incoming().take(1).flatten() {
            drop(sender.send(answer(client, greeting, reply).unwrap_or_default()));
        }
    });
    Ok(Socks { asked, url, worker })
}

#[tokio::test]
async fn socks_failures_never_fall_back_to_direct() {
    assert_eq!(refused(&[5, 255], BOUND_V4).await, "proxy:method");
    assert_eq!(
        refused(ACCEPTED, &[5, 5, 0, 1, 0, 0, 0, 0, 0, 0]).await,
        "proxy:5"
    );
    assert_eq!(refused(ACCEPTED, &[4, 0, 0, 1]).await, "io");
    assert_eq!(refused(ACCEPTED, &[5, 0, 0, 9]).await, "io");
    assert_eq!(refused(ACCEPTED, &[5, 0, 0, 3]).await, "io");
    assert_eq!(refused(&[5], BOUND_V4).await, "io");
    assert_eq!(refused(ACCEPTED, &[]).await, "io");
    let long = format!("http://{}:1/v1", "a".repeat(LONG_HOST))
        .parse::<Uri>()
        .unwrap();
    let proxy = socks(ACCEPTED, BOUND_V4).unwrap();
    assert_eq!(post(&proxy.url, &long).await.unwrap_err(), "proxy:host");
    drop(proxy.worker.join());
}

#[tokio::test]
async fn socks_reset_maps_to_io() {
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
    let proxy = format!("socks5h://127.0.0.1:{port}")
        .parse::<Uri>()
        .unwrap();
    let target = "http://127.0.0.1:1/v1".parse::<Uri>().unwrap();
    assert_eq!(post(&proxy, &target).await.unwrap_err(), "io");
    drop(worker.join());
}

#[tokio::test]
async fn socks_tunnels_with_any_bound_address() {
    for reply in [
        BOUND_V4,
        &[
            5, 0, 0, 3, 9, b'l', b'o', b'c', b'a', b'l', b'h', b'o', b's', b't', 0, 0,
        ][..],
        &[
            5, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0,
        ][..],
    ] {
        let target = serve(|_| OK_BODY.to_owned()).unwrap();
        let proxy = socks(ACCEPTED, reply).unwrap();
        let done = post(&proxy.url, &target.url).await.unwrap();
        assert_eq!(done.status(), STATUS_OK_BODY);
        let asked = proxy.asked.recv_timeout(RECV_TIMEOUT).unwrap();
        let host = target.url.host().unwrap_or_default();
        let port = target.url.port_u16().unwrap_or_default();
        let mut expected = vec![5, 1, 0, 3, u8::try_from(host.len()).unwrap()];
        expected.extend_from_slice(host.as_bytes());
        expected.extend_from_slice(&[
            u8::try_from(port >> SHIFT).unwrap(),
            u8::try_from(port & LOW).unwrap(),
        ]);
        assert_eq!(asked, expected);
        drop(proxy.worker.join());
        target.finish();
    }
}
