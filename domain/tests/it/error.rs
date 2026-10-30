use domain::AppError;

use crate::support::{Broken, ERRORS, Fixture, local};

#[test]
fn bad_request_uses_configured_code() {
    let fixture = Fixture::new().unwrap();
    let call = local("messages", "list", "").with_params("limit=x");
    let err = fixture.dispatch(&call).unwrap_err();
    assert_eq!(err.status(), 400);
    assert_eq!(err.body(fixture.errors()), r#"{"code":"invalid"}"#);
}

#[test]
fn body_escapes_code() {
    let fixture = Fixture::with(&ERRORS.replace("invalid", r#"a\"b"#)).unwrap();
    let call = local("messages", "list", "").with_params("limit=x");
    let err = fixture.dispatch(&call).unwrap_err();
    assert_eq!(err.body(fixture.errors()), r#"{"code":"a\"b"}"#);
}

#[test]
fn each_error_has_its_code_status_and_summary() {
    let fixture = Fixture::new().unwrap();
    let errors = fixture.errors();
    let cases = [
        (AppError::BadRequest, "invalid", 400, "bad request"),
        (
            AppError::Conflict,
            "clash",
            409,
            "already settled differently",
        ),
        (AppError::NotFound, "absent", 404, "not found"),
        (AppError::Storage, "unavailable", 503, "storage unavailable"),
        (AppError::UnknownAction, "missing", 404, "unknown action"),
        (
            AppError::Unprocessable,
            "refused",
            422,
            "violates review rules",
        ),
    ];
    for (err, code, status, summary) in cases {
        assert_eq!(err.body(errors), format!(r#"{{"code":"{code}"}}"#));
        assert_eq!(err.status(), status);
        assert_eq!(err.summary(), summary);
    }
}

#[test]
fn storage_uses_configured_code() {
    let fixture = Fixture::new().unwrap();
    let err = fixture
        .dispatch_with(&Broken, &local("messages", "list", ""))
        .unwrap_err();
    assert_eq!(err.status(), 503);
    assert_eq!(err.body(fixture.errors()), r#"{"code":"unavailable"}"#);
}

#[test]
fn unknown_action_uses_configured_code() {
    let fixture = Fixture::new().unwrap();
    let err = fixture.dispatch(&local("nope", "nope", "{}")).unwrap_err();
    assert_eq!(err.status(), 404);
    assert_eq!(err.body(fixture.errors()), r#"{"code":"missing"}"#);
}
