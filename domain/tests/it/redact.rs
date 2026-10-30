use core::error::Error;
use std::fs;

use serde_json::{Value, from_str as parse, json};
use serde_saphyr::from_str;

use crate::support::{Fixture, local};

const CONFIG: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../cli/config.yaml");

type Fallible<T> = Result<T, Box<dyn Error>>;

#[test]
fn keys_blocks_are_replaced_whole() {
    let block = concat!(
        "-----BEGIN RSA ",
        "PRIVATE KEY-----\nMIIEowIBAAKC\n-----END RSA ",
        "PRIVATE KEY-----\nafter"
    );
    let masked = redact(block).unwrap();
    assert!(!masked.contains("MIIEowIBAAKC") && masked.ends_with("\nafter"));
    let cut = concat!(
        "-----BEGIN OPENSSH ",
        "PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEAAAAA"
    );
    assert!(!redact(cut).unwrap().contains("b3BlbnNz"));
}

#[test]
fn plain_text_is_kept() {
    for text in [
        "hello world",
        "meeting at 10:30 tomorrow",
        "order 12345 shipped",
        "see you at the office",
        "\u{660e}\u{5929}\u{5f00}\u{4f1a}",
    ] {
        assert_eq!(redact(text).unwrap(), text);
    }
}

fn redact(text: &str) -> Fallible<String> {
    let file: Value = from_str(&fs::read_to_string(CONFIG)?)?;
    let rules = file
        .pointer("/triage/redact/patterns")
        .ok_or("no redact patterns")?;
    let request = json!({"rules": rules, "text": text}).to_string();
    let body = Fixture::new()?.dispatch(&local("triage", "redact", &request));
    let reply: Value = parse(&body.map_err(|err| format!("{err:?}"))?)?;
    Ok(reply
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned())
}

fn secrets() -> Vec<(String, &'static str)> {
    let password = "\u{5bc6}\u{7801}\u{662f}";
    let code = "\u{9a8c}\u{8bc1}\u{7801}";
    vec![
        (
            "see https://example.com/doc?token=abc123 ok".to_owned(),
            "example.com",
        ),
        ("clone git@github.com:org/repo.git".to_owned(), "github.com"),
        ("open www.example.com/a".to_owned(), "example.com"),
        ("doc alidocs.dingtalk.com/i/nodes/xyz".to_owned(), "alidocs"),
        (
            "key sk-abcdefghijklmnopqrstuvwx1234".to_owned(),
            "sk-abcdef",
        ),
        ("AKIAIOSFODNN7EXAMPLE".to_owned(), "AKIAIOSFODNN7EXAMPLE"),
        (
            "ghp_abcdefghijklmnopqrstuvwxyz0123456789".to_owned(),
            "ghp_abcdef",
        ),
        ("xoxb-1234567890-abcdefghij".to_owned(), "xoxb-1234567890"),
        (
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.abcdefghijklmnop".to_owned(),
            "eyJhbGci",
        ),
        (
            "Authorization: Bearer abcdefgh12345678".to_owned(),
            "abcdefgh12345678",
        ),
        ("Cookie: session=abc123def456".to_owned(), "abc123def456"),
        ("password: hunter2xyz".to_owned(), "hunter2xyz"),
        ("pwd=hunter2xyz".to_owned(), "hunter2xyz"),
        (format!("{password}hunter2xyz"), "hunter2xyz"),
        (format!("{code}123456"), "123456"),
        ("DB_PASSWORD=hunter2xyz".to_owned(), "hunter2xyz"),
        ("pass\u{200b}word: hunter2xyz".to_owned(), "hunter2xyz"),
        ("mail bob@example.com".to_owned(), "bob@example.com"),
        ("call 13812345678".to_owned(), "13812345678"),
        ("call 138-1234-5678".to_owned(), "1234-5678"),
        ("call +86 138 1234 5678".to_owned(), "1234 5678"),
        ("card 6222 0212 3456 7890".to_owned(), "0212 3456"),
        ("id 11010519491231002X".to_owned(), "110105194912310"),
        ("host 192.168.1.10:8080".to_owned(), "192.168.1.10"),
        ("host redis.internal:6379".to_owned(), "redis.internal"),
        ("see /Users/alice/.ssh/config".to_owned(), "alice"),
        ("file C:\\Users\\alice\\notes.txt".to_owned(), "alice"),
        ("look ~/.aws/credentials".to_owned(), ".aws"),
        ("upload server.pem now".to_owned(), "server.pem"),
        ("use id_ed25519 here".to_owned(), "id_ed25519"),
        (
            "blob aGVsbG8gd29ybGQgdGhpcyBpcyBsb25n".to_owned(),
            "aGVsbG8gd29ybGQ",
        ),
    ]
}

#[test]
fn secrets_are_replaced() {
    for (text, secret) in secrets() {
        let masked = redact(&text).unwrap();
        assert!(!masked.contains(secret), "{text} -> {masked}");
        assert_ne!(masked, text);
    }
}
