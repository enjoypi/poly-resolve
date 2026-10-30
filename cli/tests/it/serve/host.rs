use super::{raw, send, start, stop};
use crate::support::{config, live, sandbox};

#[test]
fn foreign_hosts_are_forbidden() {
    let dir = sandbox("serve-host").unwrap();
    let path = config("serve-host.yaml", &live(&dir)).unwrap();
    let server = start(&path).unwrap();
    let addr = &server.addr;
    let local = format!("Host: localhost:{}\r\n", port(addr));
    assert!(
        raw(addr, &request(&local))
            .unwrap()
            .starts_with("HTTP/1.1 200")
    );
    let bound = format!("Host: {addr}\r\n");
    assert!(
        raw(addr, &request(&bound))
            .unwrap()
            .starts_with("HTTP/1.1 200")
    );
    for host in ["Host: evil.test\r\n", "Host: localhost:1\r\n", ""] {
        let response = raw(addr, &request(host)).unwrap();
        assert!(response.starts_with("HTTP/1.1 403"), "{host}: {response}");
    }
    let page = raw(
        addr,
        "GET / HTTP/1.1\r\nHost: evil.test\r\nConnection: close\r\n\r\n",
    )
    .unwrap();
    assert!(page.starts_with("HTTP/1.1 403"));
    assert!(stop(server).unwrap());
}

fn port(addr: &str) -> &str {
    addr.rsplit(':').next().unwrap_or_default()
}

fn request(host: &str) -> String {
    format!("GET /api/health HTTP/1.1\r\n{host}Connection: close\r\n\r\n")
}

#[test]
fn writes_need_json_and_known_routes() {
    let dir = sandbox("serve-media").unwrap();
    let path = config("serve-media.yaml", &live(&dir)).unwrap();
    let server = start(&path).unwrap();
    let addr = &server.addr;
    let plain = format!(
        "PATCH /api/reviews/a HTTP/1.1\r\nHost: {addr}\r\nContent-Type: text/plain\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}"
    );
    assert!(raw(addr, &plain).unwrap().starts_with("HTTP/1.1 415"));
    let hidden = send(addr, ["GET", "/api/ingest", ""], "").unwrap();
    assert!(hidden.starts_with("HTTP/1.1 404"));
    let internal = send(addr, ["POST", "/triage/redact", ""], "{}").unwrap();
    assert!(internal.starts_with("HTTP/1.1 404"));
    let post = send(addr, ["POST", "/api/reviews", ""], "{}").unwrap();
    assert!(post.starts_with("HTTP/1.1 405"));
    assert!(stop(server).unwrap());
}
