use core::sync::atomic::{AtomicU32, Ordering};
use std::io::{self, Read as _, Write as _};
use std::net::TcpListener;
use std::thread::spawn;

use serde_json::{Value, json};

use super::fake::{hello, serve};
use super::{after, settings};
use crate::http::proxy::pipe;
use crate::support::Sandbox;

const QUICK: u64 = 200;

static SERIAL: AtomicU32 = AtomicU32::new(0);

async fn checked(url: &str, patch: &Value) -> String {
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let Ok(sandbox) = Sandbox::new(&format!("imap-check-{serial}")) else {
        return "sandbox".to_owned();
    };
    let Ok(config) = settings(&sandbox, url, patch) else {
        return "config".to_owned();
    };
    config.check().await.err().unwrap_or_default()
}

fn relay(port: u16) -> io::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let proxy = listener.local_addr()?.port();
    drop(spawn(move || {
        for mut client in listener.incoming().take(1).flatten() {
            let mut head = Vec::new();
            let mut byte = [u8::default(); 1];
            while !head.ends_with(b"\r\n\r\n") && client.read(&mut byte).unwrap_or_default() > 0 {
                head.extend_from_slice(&byte);
            }
            drop(client.write_all(b"HTTP/1.1 200 OK\r\n\r\n"));
            drop(pipe(client, &format!("127.0.0.1:{port}")));
        }
    }));
    Ok(proxy)
}

#[test]
fn sources_report_labels_and_missing_variables() {
    let sandbox = Sandbox::new("imap-vars").unwrap();
    let patch = json!({
        "proxy": "${POLY_RESOLVE_ABSENT_PROXY}",
        "user": "${POLY_RESOLVE_ABSENT_USER}",
    });
    let config = settings(&sandbox, "${POLY_RESOLVE_ABSENT_URL}", &patch).unwrap();
    assert_eq!(config.verify(), Ok(()));
    assert_eq!(
        config.missing(),
        [
            "POLY_RESOLVE_ABSENT_USER",
            "POLY_RESOLVE_ABSENT_URL",
            "POLY_RESOLVE_ABSENT_PROXY"
        ]
    );
}

#[tokio::test]
async fn startup_check_logs_in_and_out() {
    let mut script = hello("");
    script.truncate(3);
    script.push(after("LOGOUT", "* BYE\r\n<tag> OK\r\n"));
    let fake = serve(vec![script]).unwrap();
    assert_eq!(checked(&fake.url(), &json!({})).await, "");
    assert_eq!(fake.finish().last().map(String::as_str), Some("LOGOUT"));
}

#[tokio::test]
async fn startup_check_names_the_failing_stage() {
    let url = "imap:work: imap:url";
    assert!(
        checked("imap://example.com:1", &json!({}))
            .await
            .starts_with(url)
    );
    assert!(
        checked("imaps://localhost:1", &json!({}))
            .await
            .starts_with(url)
    );
    assert!(
        checked("ftp://127.0.0.1:1", &json!({}))
            .await
            .starts_with(url)
    );
    assert!(checked("::bad", &json!({})).await.starts_with(url));
    let plain = json!({"proxy": "http://127.0.0.1:9"});
    assert!(checked("imap://127.0.0.1:1", &plain).await.starts_with(url));
    let outside = json!({"proxy": "http://example.com:9"});
    let proxy = checked("imap://127.0.0.1:1", &outside).await;
    assert!(proxy.starts_with("imap:work: proxy must be"));
    let refused = checked("imap://127.0.0.1:1", &json!({})).await;
    assert!(refused.starts_with("imap:work: connect"));
}

#[tokio::test]
async fn startup_check_rejects_unusable_credentials() {
    for field in ["user", "password"] {
        let mut script = hello("");
        script.truncate(2);
        let fake = serve(vec![script]).unwrap();
        let patch = json!({ field: "${POLY_RESOLVE_TEST_NON_ASCII}" });
        assert_eq!(
            checked(&fake.url(), &patch).await,
            "imap:work: imap:credential"
        );
        drop(fake.finish());
    }
}

#[tokio::test]
async fn tls_is_required_through_proxies() {
    let fake = serve(vec![hello("")]).unwrap();
    let proxy = relay(fake.port()).unwrap();
    let patch = json!({"proxy": format!("http://127.0.0.1:{proxy}")});
    assert_eq!(
        checked("${POLY_RESOLVE_TEST_IMAPS}", &patch).await,
        "imap:work: tls"
    );
    drop(fake.finish());
    let silent = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = silent.local_addr().unwrap().port();
    let socks = json!({"proxy": format!("socks5://127.0.0.1:{port}"), "timeout_ms": QUICK});
    assert_eq!(
        checked("${POLY_RESOLVE_TEST_IMAPS}", &socks).await,
        "imap:work: timeout:proxy"
    );
    drop(silent);
}
