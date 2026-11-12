use domain::AppError;
use domain::topics::Board;
use serde_json::{Error, Value, from_str, json};

use crate::support::{Broken, Desk, Fixture, KNOWN, local};

const LINES: u64 = 3;

type Reply = Result<Value, u16>;

fn get(topic: &str, board: &dyn Board) -> Result<Reply, Error> {
    let params = format!("topic_id={topic}");
    let call = local("topics", "get", "").with_params(&params);
    reply(Fixture::new()?.dispatch_board(board, &call))
}

#[test]
fn get_returns_one_topic_or_not_found() {
    let desk = Desk::new().unwrap();
    let found = get(KNOWN, &desk).unwrap().unwrap();
    assert_eq!(found.pointer("/topic/label"), Some(&json!("outage")));
    assert_eq!(found.get("timeline"), Some(&json!([])));
    assert_eq!(get("other", &desk).unwrap(), Err(404));
    assert_eq!(get(KNOWN, &Broken).unwrap(), Err(503));
    let call = local("topics", "get", "");
    assert_eq!(
        reply(Fixture::new().unwrap().dispatch_board(&desk, &call)).unwrap(),
        Err(400)
    );
}

fn list(query: &str, board: &dyn Board) -> Result<Reply, Error> {
    let call = local("topics", "list", "").with_params(query);
    reply(Fixture::new()?.dispatch_board(board, &call))
}

#[test]
fn list_returns_topics_and_tallies() {
    let desk = Desk::new().unwrap();
    let all = list("", &desk).unwrap().unwrap();
    assert_eq!(all.pointer("/items/0/topic_id"), Some(&json!(KNOWN)));
    assert_eq!(all.pointer("/lines/0/count"), Some(&json!(LINES)));
    assert_eq!(all.pointer("/topics/0/state"), Some(&json!("open")));
    let closed = list("state=resolved&limit=1", &desk).unwrap().unwrap();
    assert_eq!(closed.get("items"), Some(&json!([])));
    assert_eq!(list("limit=0", &desk).unwrap(), Err(400));
    assert_eq!(list("limit=x", &desk).unwrap(), Err(400));
    assert_eq!(list("", &Broken).unwrap(), Err(503));
}

#[test]
fn missing_board_is_empty() {
    let fixture = Fixture::new().unwrap();
    let listed = reply(fixture.dispatch(&local("topics", "list", ""))).unwrap();
    assert_eq!(listed, Ok(json!({"items": [], "lines": [], "topics": []})));
    let call = local("topics", "get", "").with_params("topic_id=T1");
    assert_eq!(reply(fixture.dispatch(&call)).unwrap(), Err(404));
}

fn reply(result: Result<String, AppError>) -> Result<Reply, Error> {
    match result {
        Ok(response) => Ok(Ok(from_str(&response)?)),
        Err(err) => Ok(Err(err.status())),
    }
}
