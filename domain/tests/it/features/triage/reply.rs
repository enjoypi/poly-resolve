use serde_json::{Error, Value, json};

use super::{Reply, call};

const ROOMY: u32 = 100;
const START: &str = r#"{"type":"thread.started","thread_id":"t-1"}"#;
const TOOL: &str =
    r#"{"type":"item.completed","item":{"id":"i3","type":"command_execution","text":"ls"}}"#;

fn agent(text: &str) -> String {
    json!({"type": "item.completed", "item": {"id": "i", "type": "agent_message", "text": text}})
        .to_string()
}

fn headed(header: &Value, body: &str) -> String {
    format!("{START}\n{}\n", agent(&format!("{header}\n{body}")))
}

fn reply(stdout: &str, max_chars: u32) -> Result<Reply, Error> {
    reply_after(stdout, max_chars, "")
}

fn reply_after(stdout: &str, max_chars: u32, state: &str) -> Result<Reply, Error> {
    let request = json!({"max_chars": max_chars, "state": state, "stdout": stdout}).to_string();
    call("reply", &request)
}

#[test]
fn reply_keeps_quiet_only_when_nothing_changed() {
    let same = headed(&json!({"changed": false, "status": "open"}), "body");
    let quiet = reply_after(&same, ROOMY, "open").unwrap().unwrap();
    assert_eq!(quiet.get("outcome"), Some(&json!("quiet")));
    let closed = reply_after(&same, ROOMY, "resolved").unwrap().unwrap();
    assert_eq!(closed.get("outcome"), Some(&json!("notify")));
    let moved = headed(&json!({"changed": true, "status": "open"}), "body");
    let loud = reply_after(&moved, ROOMY, "open").unwrap().unwrap();
    assert_eq!(loud.get("outcome"), Some(&json!("notify")));
    let silent = headed(&json!({"status": "open"}), "body");
    let unsure = reply_after(&silent, ROOMY, "open").unwrap().unwrap();
    assert_eq!(unsure.get("outcome"), Some(&json!("notify")));
}

#[test]
fn reply_rejects_malformed_request() {
    assert_eq!(call("reply", "nope").unwrap(), Err(400));
    assert_eq!(call("reply", r#"{"max_chars":1}"#).unwrap(), Err(400));
}

#[test]
fn reply_rejects_streams_without_thread_or_answer() {
    for stdout in [
        String::new(),
        "not json\n".to_owned(),
        format!("{START}\n"),
        format!("{START}\n{TOOL}\n"),
        format!("{}\n", agent("{\"status\":\"open\"}\nbody")),
        format!("{START}\n{}\n", agent("  ")),
    ] {
        assert_eq!(reply(&stdout, ROOMY).unwrap(), Err(400), "{stdout}");
    }
    for stdout in [
        format!("{START}\n{}\n", agent("no header")),
        format!("{START}\n{}\n", agent("not json\nbody")),
        format!("{START}\n{}\n", agent("{\"status\":\"open\"}\n  ")),
        format!("{START}\n{}\n", agent("{\"status\":\"odd\"}\nbody")),
        format!("{START}\n{}\n", agent("{\"title\":\"x\"}\nbody")),
        format!(
            "{START}\n{}\n",
            r#"{"type":"item.completed","item":{"type":"agent_message"}}"#
        ),
    ] {
        assert_eq!(reply(&stdout, ROOMY).unwrap(), Err(400), "{stdout}");
    }
}

#[test]
fn reply_takes_thread_header_and_last_answer() {
    let header =
        json!({"changed": true, "changes": "moved", "status": "resolved", "title": "outage"});
    let stdout = format!(
        "noise\n{START}\n{}\n{TOOL}\n{}\n{{\"type\":\"turn.completed\"}}\n",
        agent("{\"status\":\"open\"}\nfirst"),
        agent(&format!("  {header}\n final answer \n")),
    );
    let done = reply(&stdout, ROOMY).unwrap().unwrap();
    assert_eq!(
        done,
        json!({
            "changes": "moved",
            "outcome": "notify",
            "status": "resolved",
            "text": "final answer",
            "thread_id": "t-1",
            "title": "outage",
        })
    );
}

#[test]
fn reply_truncates_by_characters() {
    let header = json!({"changes": "abcdefgh", "status": "open", "title": "abcdefgh"});
    let done = reply(&headed(&header, "final answer"), 5).unwrap().unwrap();
    assert_eq!(done.get("text"), Some(&json!("final")));
    assert_eq!(done.get("changes"), Some(&json!("abcde")));
    assert_eq!(done.get("title"), Some(&json!("abcde")));
}

#[test]
fn started_finds_the_thread_of_a_failed_run() {
    let found = call(
        "started",
        &json!({"stdout": format!("noise\n{START}\n")}).to_string(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(found, json!({"thread_id": "t-1"}));
    let none = call("started", &json!({"stdout": TOOL}).to_string())
        .unwrap()
        .unwrap();
    assert_eq!(none, json!({"thread_id": null}));
    assert_eq!(call("started", "nope").unwrap(), Err(400));
}
