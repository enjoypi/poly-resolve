use domain::{AppError, Errors};
use serde_json::from_str;

use crate::support::{Broken, ERRORS, Fixture, cli};

#[test]
fn bad_request_uses_configured_code() {
    let fixture = Fixture::new().unwrap();
    let errors = fixture.errors();
    let err = fixture
        .dispatch(&cli("health", "check", "not json"))
        .unwrap_err();
    assert_eq!(err.status(), 400);
    assert_eq!(err.exit_code(errors), 13);
    assert_eq!(err.body(errors), r#"{"code":"invalid"}"#);
}

#[test]
fn body_escapes_code() {
    let fixture = Fixture::with(&ERRORS.replace("invalid", r#"a\"b"#)).unwrap();
    let err = fixture
        .dispatch(&cli("health", "check", "not json"))
        .unwrap_err();
    assert_eq!(err.body(fixture.errors()), r#"{"code":"a\"b"}"#);
}

#[test]
fn rejects_zero_exit_code() {
    let err = from_str::<Errors>(&ERRORS.replace("13", "0")).unwrap_err();
    assert!(err.to_string().contains("nonzero"));
}

#[test]
fn storage_uses_configured_code() {
    let fixture = Fixture::new().unwrap();
    let errors = fixture.errors();
    let err = fixture
        .dispatch_with(&Broken, &cli("messages", "list", "{}"))
        .unwrap_err();
    assert_eq!(err.status(), 503);
    assert_eq!(err.exit_code(errors), 15);
    assert_eq!(err.body(errors), r#"{"code":"unavailable"}"#);
}

#[test]
fn summaries_describe_each_error() {
    assert_eq!(AppError::BadRequest.summary(), "bad request");
    assert_eq!(AppError::Storage.summary(), "storage unavailable");
    assert_eq!(AppError::UnknownAction.summary(), "unknown action");
}

#[test]
fn unknown_action_uses_configured_code() {
    let fixture = Fixture::new().unwrap();
    let errors = fixture.errors();
    let err = fixture.dispatch(&cli("nope", "nope", "{}")).unwrap_err();
    assert_eq!(err.status(), 404);
    assert_eq!(err.exit_code(errors), 14);
    assert_eq!(err.body(errors), r#"{"code":"missing"}"#);
}
