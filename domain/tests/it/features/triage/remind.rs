use serde_json::json;

use super::call;

const NOON: u64 = 1_783_483_235_000;

#[test]
fn remind_fills_names_values_and_times() {
    let request = json!({
        "names": {"down": "lost"},
        "template": "{source} {state} {since} {streak} {gone}",
        "times": {"since": NOON},
        "values": {"source": "imap", "state": "down", "streak": "3"},
    });
    let found = call("remind", &request.to_string()).unwrap().unwrap();
    assert_eq!(
        found,
        json!({"text": "imap lost 2026-07-08 04:00:35 UTC 3 {gone}"})
    );
    let huge = json!({"template": "{at}", "times": {"at": u64::MAX}});
    let blank = call("remind", &huge.to_string()).unwrap().unwrap();
    assert_eq!(blank, json!({"text": ""}));
    assert_eq!(call("remind", "{}").unwrap(), Err(400));
}
