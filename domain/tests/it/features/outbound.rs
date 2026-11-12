use domain::AppError;
use domain::outbound::Outbox;
use serde_json::{Error, Value, from_str, json};

use crate::support::{Broken, Fixture, Sent, local};

type Reply = Result<Value, u16>;

fn keys(reply: &Value) -> Vec<&str> {
    reply
        .get("items")
        .and_then(Value::as_array)
        .map_or_default(|items| {
            items
                .iter()
                .filter_map(|item| item.get("key").and_then(Value::as_str))
                .collect()
        })
}

fn list(query: &str, outbox: &dyn Outbox) -> Result<Reply, Error> {
    let call = local("outbound", "list", "").with_params(query);
    reply(Fixture::new()?.dispatch_outbox(outbox, &call))
}

#[test]
fn list_defaults_to_approved_and_returns_placeholders() {
    let reply = list("", &sent()).unwrap().unwrap();
    assert_eq!(keys(&reply), ["k1", "k3"]);
    assert_eq!(reply.get("placeholders"), Some(&json!(["[x]"])));
}

#[test]
fn list_filters_by_state_and_limit() {
    let withheld = list("state=withheld", &sent()).unwrap().unwrap();
    assert_eq!(keys(&withheld), ["k2"]);
    let one = list("limit=1", &sent()).unwrap().unwrap();
    assert_eq!(keys(&one), ["k1"]);
}

#[test]
fn list_rejects_malformed_request() {
    assert_eq!(list("limit=x", &sent()).unwrap(), Err(400));
    assert_eq!(list("limit=0", &sent()).unwrap(), Err(400));
    assert_eq!(list("limit=4", &sent()).unwrap(), Err(400));
}

#[test]
fn list_reports_storage_failure() {
    assert_eq!(list("", &Broken).unwrap(), Err(503));
}

fn reply(result: Result<String, AppError>) -> Result<Reply, Error> {
    match result {
        Ok(response) => Ok(Ok(from_str(&response)?)),
        Err(err) => Ok(Err(err.status())),
    }
}

fn sent() -> Sent {
    Sent::new(&[("k1", "approved"), ("k2", "withheld"), ("k3", "approved")])
}
