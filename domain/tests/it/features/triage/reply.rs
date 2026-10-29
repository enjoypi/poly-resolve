use serde_json::{Error, json};

use super::{Reply, call};

const AGENT: &str =
    r#"{"type":"item.completed","item":{"id":"i1","type":"agent_message","text":"  first  "}}"#;
const LATER: &str =
    r#"{"type":"item.completed","item":{"id":"i2","type":"agent_message","text":"final answer"}}"#;
const ROOMY: u32 = 100;
const START: &str = r#"{"type":"thread.started","thread_id":"t-1"}"#;
const TOOL: &str =
    r#"{"type":"item.completed","item":{"id":"i3","type":"command_execution","text":"ls"}}"#;

fn reply(stdout: &str, max_chars: u32) -> Result<Reply, Error> {
    let request = json!({"max_chars": max_chars, "stdout": stdout}).to_string();
    call("reply", &request)
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
        format!("{AGENT}\n"),
        format!(
            "{START}\n{}\n",
            r#"{"type":"item.completed","item":{"type":"agent_message","text":"  "}}"#
        ),
        format!(
            "{START}\n{}\n",
            r#"{"type":"item.completed","item":{"type":"agent_message"}}"#
        ),
    ] {
        assert_eq!(reply(&stdout, ROOMY).unwrap(), Err(400), "{stdout}");
    }
}

#[test]
fn reply_takes_thread_and_last_answer() {
    let stdout =
        format!("noise\n{START}\n{AGENT}\n{TOOL}\n{LATER}\n{{\"type\":\"turn.completed\"}}\n");
    let done = reply(&stdout, ROOMY).unwrap().unwrap();
    assert_eq!(done.get("thread_id"), Some(&json!("t-1")));
    assert_eq!(done.get("text"), Some(&json!("final answer")));
}

#[test]
fn reply_truncates_by_characters() {
    let stdout = format!("{START}\n{LATER}\n");
    let done = reply(&stdout, 5).unwrap().unwrap();
    assert_eq!(done.get("text"), Some(&json!("final")));
}
