use crate::support::{Fixture, local};

#[test]
fn check_returns_ok_without_reading_the_request() {
    let fixture = Fixture::new().unwrap();
    for body in ["", "not json"] {
        let reply = fixture.dispatch(&local("health", "check", body)).unwrap();
        assert_eq!(reply, r#"{"status":"ok"}"#);
    }
}
