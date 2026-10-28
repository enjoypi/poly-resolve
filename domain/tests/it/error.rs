use domain::Errors;
use domain::features::dispatch;
use serde_json::from_str;

use crate::support::{ERRORS, cli, errors};

#[test]
fn bad_request_uses_configured_code() {
    let errors = errors().unwrap();
    let err = dispatch(&errors, &cli("health", "check", "not json")).unwrap_err();
    assert_eq!(err.status(), 400);
    assert_eq!(err.exit_code(&errors), 13);
    assert_eq!(err.body(&errors), r#"{"code":"invalid"}"#);
}

#[test]
fn body_escapes_code() {
    let errors: Errors = from_str(&ERRORS.replace("invalid", r#"a\"b"#)).unwrap();
    let err = dispatch(&errors, &cli("health", "check", "not json")).unwrap_err();
    assert_eq!(err.body(&errors), r#"{"code":"a\"b"}"#);
}

#[test]
fn rejects_zero_exit_code() {
    let err = from_str::<Errors>(&ERRORS.replace("13", "0")).unwrap_err();
    assert!(err.to_string().contains("nonzero"));
}

#[test]
fn unknown_action_uses_configured_code() {
    let errors = errors().unwrap();
    let err = dispatch(&errors, &cli("nope", "nope", "{}")).unwrap_err();
    assert_eq!(err.status(), 404);
    assert_eq!(err.exit_code(&errors), 14);
    assert_eq!(err.body(&errors), r#"{"code":"missing"}"#);
}
