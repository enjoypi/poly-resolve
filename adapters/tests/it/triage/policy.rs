use domain::review::Draw;
use serde_json::json;

use crate::support::Sandbox;

const TEN: u32 = 10;

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
