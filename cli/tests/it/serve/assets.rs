use std::fs;
use std::io;

use super::{get, post, start, stop};
use crate::support::{config, live, sandbox};

const INDEX: &str = "<h1>home</h1>";

fn check(addr: &str, target: &str, expected: [&str; 2]) -> io::Result<bool> {
    let [kind, body] = expected;
    let response = get(addr, target)?;
    Ok(response.starts_with("HTTP/1.1 200")
        && response.contains(&format!("\r\ncontent-type: {kind}\r\n"))
        && response.ends_with(body))
}

#[test]
fn hides_unsafe_and_missing_paths() {
    let path = site("cli-assets-hidden").unwrap();
    let server = start(&path).unwrap();
    for target in [
        "/missing.js",
        "/.hidden",
        "/..",
        "/%2e%2e%2fsecret.txt",
        "/a%5Cb",
    ] {
        let response = get(&server.addr, target).unwrap();
        assert!(response.starts_with("HTTP/1.1 404"), "{target}: {response}");
        assert!(!response.contains("secret"), "{target}: {response}");
        assert!(!response.contains("hidden"), "{target}: {response}");
    }
    let method = get(&server.addr, "/health/check").unwrap();
    assert!(method.starts_with("HTTP/1.1 405"));
    let write = post(&server.addr, ["/app.js", ""], "").unwrap();
    assert!(write.starts_with("HTTP/1.1 405"));
    assert!(stop(server).unwrap());
}

#[test]
fn missing_web_dir_gives_not_found() {
    let dir = sandbox("cli-assets-none").unwrap();
    let path = config("cli-assets-none.yaml", &live(&dir)).unwrap();
    let server = start(&path).unwrap();
    let response = get(&server.addr, "/").unwrap();
    assert!(response.starts_with("HTTP/1.1 404"));
    assert!(stop(server).unwrap());
}

#[test]
fn serves_site_files_with_content_types() {
    let path = site("cli-assets-files").unwrap();
    let server = start(&path).unwrap();
    let html = "text/html; charset=utf-8";
    let binary = "application/octet-stream";
    let addr = &server.addr;
    assert!(check(addr, "/", [html, INDEX]).unwrap(), "/");
    assert!(check(addr, "/index.html", [html, INDEX]).unwrap(), "index");
    let script = ["text/javascript; charset=utf-8", "run()"];
    assert!(check(addr, "/app.js", script).unwrap(), "script");
    let style = ["text/css; charset=utf-8", "body{}"];
    assert!(check(addr, "/style.css", style).unwrap(), "style");
    assert!(check(addr, "/data.bin", [binary, "bin"]).unwrap(), "binary");
    assert!(check(addr, "/LICENSE", [binary, "text"]).unwrap(), "plain");
    assert!(stop(server).unwrap());
}

fn site(name: &str) -> io::Result<String> {
    let root = sandbox(name)?;
    let dir = format!("{root}/web");
    fs::create_dir_all(&dir)?;
    for (file, body) in [
        ("index.html", INDEX),
        ("app.js", "run()"),
        ("style.css", "body{}"),
        ("data.bin", "bin"),
        ("LICENSE", "text"),
        (".hidden", "hidden"),
    ] {
        fs::write(format!("{dir}/{file}"), body)?;
    }
    fs::write(format!("{root}/secret.txt"), "secret")?;
    config(
        &format!("{name}.yaml"),
        &live(&root).replace("/nonexistent/web", &dir),
    )
}
