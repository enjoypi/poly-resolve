use domain::AppError;
use domain::gauge::{Day, Health, Meter};
use serde_json::{Error, Value, from_str, from_value, json};

use crate::support::{Broken, Fixture, local};

const CALLS: u64 = 4;
const ONE: u64 = 1;
const STREAK: u64 = 2;

#[derive(Debug)]
struct Dial;

impl Meter for Dial {
    fn sources(&self) -> Result<Health, AppError> {
        from_value(json!({
            "sources": [{"account": "a", "down_at_ms": ONE, "last_at_ms": null, "kind": "imap", "ok_at_ms": ONE, "state": "ok", "streak": ONE}],
            "stages": [{"kind": "decision", "streak": STREAK}],
        }))
        .map_err(|_err| AppError::Storage)
    }

    fn usage(&self, days: u32) -> Result<Vec<Day>, AppError> {
        let day = json!({"calls": CALLS, "date": format!("d{days}"), "failures": ONE, "input_tokens": CALLS, "kind": "analysis", "output_tokens": ONE});
        from_value(json!([day])).map_err(|_err| AppError::Storage)
    }
}

type Reply = Result<Value, u16>;

fn ask(feature: &str, query: &str, gauge: &dyn Meter) -> Result<Reply, Error> {
    let call = local(feature, "list", "").with_params(query);
    match Fixture::new()?.dispatch_gauge(gauge, &call) {
        Ok(response) => Ok(Ok(from_str(&response)?)),
        Err(err) => Ok(Err(err.status())),
    }
}

#[test]
fn missing_gauge_is_empty() {
    let fixture = Fixture::new().unwrap();
    let health = fixture.dispatch(&local("sources", "list", "")).unwrap();
    assert_eq!(
        from_str::<Value>(&health).unwrap(),
        json!({"sources": [], "stages": []})
    );
    let days = fixture.dispatch(&local("usage", "list", "")).unwrap();
    assert_eq!(from_str::<Value>(&days).unwrap(), json!({"days": []}));
}

#[test]
fn sources_report_each_source_and_stage() {
    let found = ask("sources", "", &Dial).unwrap().unwrap();
    assert_eq!(found.pointer("/sources/0/kind"), Some(&json!("imap")));
    assert_eq!(found.pointer("/stages/0/streak"), Some(&json!(STREAK)));
    assert_eq!(ask("sources", "", &Broken).unwrap(), Err(503));
}

#[test]
fn usage_lists_days_within_the_limit() {
    let found = ask("usage", "limit=3", &Dial).unwrap().unwrap();
    assert_eq!(found.pointer("/days/0/date"), Some(&json!("d3")));
    assert_eq!(found.pointer("/days/0/calls"), Some(&json!(CALLS)));
    let fallback = ask("usage", "", &Dial).unwrap().unwrap();
    assert_eq!(fallback.pointer("/days/0/date"), Some(&json!("d2")));
    assert_eq!(ask("usage", "limit=0", &Dial).unwrap(), Err(400));
    assert_eq!(ask("usage", "limit=x", &Dial).unwrap(), Err(400));
    assert_eq!(ask("usage", "", &Broken).unwrap(), Err(503));
}
