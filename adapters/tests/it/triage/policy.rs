use domain::review::Draw;
use serde_json::json;

use crate::support::Sandbox;

const TEN: u32 = 10;
const ZERO: u32 = 0;

#[test]
fn draw_is_offered_only_for_manual_review() {
    let sandbox = Sandbox::new("policy-draw").unwrap();
    let manual = json!({"review": {"mode": "manual"}});
    assert_eq!(
        shape(sandbox.triage(&manual).unwrap().draw()),
        Some((TEN, "pending"))
    );
    let mut screened = sandbox.screen("");
    if let Some(map) = screened.as_object_mut() {
        drop(map.insert("review".to_owned(), json!({"mode": "manual"})));
    }
    assert_eq!(
        shape(sandbox.triage(&screened).unwrap().draw()),
        Some((TEN, "screening"))
    );
    assert_eq!(shape(sandbox.triage(&json!({})).unwrap().draw()), None);
    let off = json!({"mode": "disabled", "review": {"mode": "manual"}});
    assert_eq!(shape(sandbox.triage(&off).unwrap().draw()), None);
}

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
}

#[test]
fn marks_collect_every_placeholder() {
    let sandbox = Sandbox::new("policy-marks").unwrap();
    assert_eq!(
        sandbox.triage(&json!({})).unwrap().marks(),
        ["[num]", "[x]"]
    );
    let screened = sandbox.triage(&sandbox.screen("")).unwrap();
    assert_eq!(screened.marks(), ["[num]", "[s]", "[x]"]);
}

fn shape(policy: Draw) -> Option<(u32, &'static str)> {
    policy.pick().map(|pick| (pick.limit(), pick.state()))
}
