use domain::features::dispatch;

use crate::support::{cli, errors};

#[test]
fn check_returns_ok() {
    let body = dispatch(&errors().unwrap(), &cli("health", "check", "{}")).unwrap();
    assert_eq!(body, r#"{"status":"ok"}"#);
}
