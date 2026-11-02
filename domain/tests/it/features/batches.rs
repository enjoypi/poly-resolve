use domain::AppError;
use domain::batches::Roster;
use serde_json::{Error, Value, from_str, json};

use crate::support::{Broken, Fixture, Pool, local};

const READY: &str = r#"{"state":"ready"}"#;

type Reply = Result<Value, u16>;

fn ids(reply: &Value) -> Vec<u64> {
    reply
        .get("items")
        .and_then(Value::as_array)
        .map_or_default(|items| {
            items
                .iter()
                .filter_map(|item| item.get("batch_id").and_then(Value::as_u64))
                .collect()
        })
}

fn list(query: &str, roster: &dyn Roster) -> Result<Reply, Error> {
    let call = local("batches", "list", "").with_params(query);
    reply(Fixture::new()?.dispatch_roster(roster, &call))
}

#[test]
fn list_defaults_to_failed_batches() {
    let reply = list("", &pool()).unwrap().unwrap();
    assert_eq!(ids(&reply), [1, 3]);
    let sent = list("state=sent&limit=1", &pool()).unwrap().unwrap();
    assert_eq!(ids(&sent), [2]);
}

#[test]
fn list_rejects_malformed_request() {
    assert_eq!(list("limit=x", &pool()).unwrap(), Err(400));
    assert_eq!(list("limit=0", &pool()).unwrap(), Err(400));
}

#[test]
fn list_reports_storage_failure() {
    assert_eq!(list("", &Broken).unwrap(), Err(503));
}

fn pool() -> Pool {
    Pool::new(&[(1, "failed"), (2, "sent"), (3, "failed"), (4, "ready")])
}

fn reply(result: Result<String, AppError>) -> Result<Reply, Error> {
    match result {
        Ok(response) => Ok(Ok(from_str(&response)?)),
        Err(err) => Ok(Err(err.status())),
    }
}

fn update(roster: &dyn Roster, query: &str, body: &str) -> Result<Reply, Error> {
    let call = local("batches", "update", body).with_params(query);
    reply(Fixture::new()?.dispatch_roster(roster, &call))
}

#[test]
fn update_rejects_malformed_requests() {
    assert_eq!(update(&pool(), "batch_id=x", READY).unwrap(), Err(400));
    let wrong = r#"{"state":"sent"}"#;
    assert_eq!(update(&pool(), "batch_id=1", wrong).unwrap(), Err(400));
}

#[test]
fn update_reports_conflicts_and_missing_batches() {
    assert_eq!(update(&pool(), "batch_id=2", READY).unwrap(), Err(409));
    assert_eq!(update(&pool(), "batch_id=9", READY).unwrap(), Err(404));
    assert_eq!(update(&Broken, "batch_id=1", READY).unwrap(), Err(503));
}

#[test]
fn update_revives_failed_batches() {
    let revived = update(&pool(), "batch_id=1", READY).unwrap().unwrap();
    assert_eq!(revived, json!({"state": "ready"}));
    let again = update(&pool(), "batch_id=4", READY).unwrap().unwrap();
    assert_eq!(again, json!({"state": "ready"}));
}
