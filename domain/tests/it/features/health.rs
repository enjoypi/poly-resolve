use crate::support::{Fixture, cli};

#[test]
fn check_returns_ok() {
    let body = Fixture::new()
        .unwrap()
        .dispatch(&cli("health", "check", "{}"))
        .unwrap();
    assert_eq!(body, r#"{"status":"ok"}"#);
}
