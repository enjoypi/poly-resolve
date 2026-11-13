use serde_json::json;

use crate::support::Sandbox;

const ZERO: u32 = 0;

#[test]
fn idle_screening_and_endless_waits_are_rejected() {
    let sandbox = Sandbox::new("policy-zero").unwrap();
    let mut idle = sandbox.screen("");
    if let Some(screen) = idle
        .get_mut("screen")
        .and_then(|found| found.as_object_mut())
    {
        drop(screen.insert("parallel".to_owned(), json!(ZERO)));
    }
    let refused = sandbox.triage(&idle).unwrap_err().to_string();
    assert!(refused.contains("nonzero"), "{refused}");
    let endless = json!({"limits": {"retry_max_ms": ZERO}});
    let unbounded = sandbox.triage(&endless).unwrap_err().to_string();
    assert!(unbounded.contains("nonzero"), "{unbounded}");
    let boundless = json!({"topics": {"clues": {"max_conversations": ZERO}}});
    let unlimited = sandbox.triage(&boundless).unwrap_err().to_string();
    assert!(unlimited.contains("nonzero"), "{unlimited}");
}

#[test]
fn marks_collect_every_placeholder() {
    let sandbox = Sandbox::new("policy-marks").unwrap();
    assert_eq!(sandbox.triage(&json!({})).unwrap().marks(), ["[num]"]);
    let screened = sandbox.triage(&sandbox.screen("")).unwrap();
    assert_eq!(screened.marks(), ["[num]", "[s]"]);
}
