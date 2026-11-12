use std::fs;

use domain::prompts::{Library as _, Prompt};
use serde_json::{Value, json};

use super::rig::{ALERTS, BATCHES, NOTHING, QUICK, SHORT, calls, logged, one, seed, start, table};
use crate::support::{Fallible, Sandbox, triage_validate};

type Entry = (String, String);

#[tokio::test]
async fn edited_prompts_apply_on_the_next_call() {
    let sandbox = Sandbox::new("prompts-hot").unwrap();
    seed(&sandbox, &one()).unwrap();
    let dir = sandbox.prompts().unwrap();
    fs::write(dir.join("session.md"), "fresh:{messages}").unwrap();
    fs::write(dir.join("urgent.md"), "hot {line}").unwrap();
    start(&sandbox, &json!({}), SHORT).await.unwrap();
    assert!(
        sandbox
            .read("codex-stdin.txt")
            .unwrap()
            .starts_with("fresh:")
    );
    assert!(sandbox.read("decide-body.txt").unwrap().contains("hot L1"));
}

#[test]
fn folder_lists_configured_prompts_and_saves_atomically() {
    let sandbox = Sandbox::new("prompts-folder").unwrap();
    let names: Vec<String> = listed(&sandbox, &json!({}))
        .unwrap()
        .into_iter()
        .map(|(name, _text)| name)
        .collect();
    assert_eq!(names, ["followup", "review", "session", "topic", "urgent"]);
    let screened = listed(&sandbox, &sandbox.screen("")).unwrap();
    assert_eq!(
        screened.last(),
        Some(&("screen".to_owned(), "judge".to_owned()))
    );
    let folder = sandbox.triage(&json!({})).unwrap().prompts();
    let fresh = Prompt::new("urgent".to_owned(), "now {line}".to_owned());
    assert_eq!(folder.save(&fresh).unwrap(), Some(()));
    assert_eq!(sandbox.read("prompts/urgent.md").unwrap(), "now {line}");
    assert!(!sandbox.prompts().unwrap().join("urgent.md.tmp").exists());
    let stray = Prompt::new("screen".to_owned(), "judge".to_owned());
    assert_eq!(folder.save(&stray).unwrap(), None);
}

#[test]
fn folder_reports_storage_failures() {
    let sandbox = Sandbox::new("prompts-folder-broken").unwrap();
    let dir = sandbox.prompts().unwrap();
    let folder = sandbox.triage(&json!({})).unwrap().prompts();
    fs::create_dir_all(dir.join("session.md.tmp")).unwrap();
    let session = Prompt::new("session".to_owned(), "a {messages}".to_owned());
    assert_eq!(folder.save(&session).unwrap_err().status(), 503);
    fs::remove_file(dir.join("topic.md")).unwrap();
    fs::create_dir_all(dir.join("topic.md")).unwrap();
    let topic = Prompt::new("topic".to_owned(), "b {segment}".to_owned());
    assert_eq!(folder.save(&topic).unwrap_err().status(), 503);
    assert!(!dir.join("topic.md.tmp").exists());
    assert_eq!(folder.list().unwrap_err().status(), 503);
}

fn listed(sandbox: &Sandbox, patch: &Value) -> Fallible<Vec<Entry>> {
    let folder = sandbox.triage(patch)?.prompts();
    let found = folder
        .list()
        .map_err(|err| format!("status {}", err.status()))?;
    Ok(found
        .iter()
        .map(|prompt| (prompt.name().to_owned(), prompt.text().to_owned()))
        .collect())
}

#[tokio::test]
async fn unusable_analysis_and_screen_prompts_hold_the_work() {
    for file in ["followup.md", "session.md"] {
        let sandbox = Sandbox::new(&format!("prompts-analysis-{file}")).unwrap();
        seed(&sandbox, &one()).unwrap();
        fs::remove_file(sandbox.prompts().unwrap().join(file)).unwrap();
        let logs = logged(&sandbox, &json!({}), SHORT).await.unwrap();
        assert!(logs.contains("prompt unreadable"), "{file}");
        assert_eq!(calls(&sandbox, "codex").unwrap(), 0, "{file}");
        assert_eq!(
            table(&sandbox, ALERTS).unwrap(),
            ["t1-0@1|pending|0||"],
            "{file}"
        );
    }
    let screened = Sandbox::new("prompts-screen").unwrap();
    seed(&screened, &one()).unwrap();
    fs::write(screened.prompts().unwrap().join("screen.md"), " ").unwrap();
    let screening = logged(&screened, &screened.screen(""), QUICK)
        .await
        .unwrap();
    assert!(screening.contains("prompt rejected") && screening.contains("screening failed"));
    assert_eq!(calls(&screened, "screen").unwrap(), 0);
}

#[tokio::test]
async fn unusable_prompts_hold_the_work() {
    for (name, file, text) in [
        ("prompts-missing", "urgent.md", None),
        ("prompts-invalid", "urgent.md", Some("no placeholder")),
        ("prompts-topic", "topic.md", Some("no placeholder")),
        ("prompts-review", "review.md", None),
    ] {
        let sandbox = Sandbox::new(name).unwrap();
        seed(&sandbox, &one()).unwrap();
        let path = sandbox.prompts().unwrap().join(file);
        match text {
            Some(body) => fs::write(&path, body).unwrap(),
            None => fs::remove_file(&path).unwrap(),
        }
        let logs = logged(&sandbox, &json!({}), QUICK).await.unwrap();
        assert!(logs.contains("batch sealed"), "{name}");
        assert_eq!(table(&sandbox, BATCHES).unwrap(), NOTHING, "{name}");
        assert_eq!(calls(&sandbox, "decide").unwrap(), 0, "{name}");
        assert!(!logs.contains("no placeholder"), "{name}");
    }
}

#[test]
fn validate_names_the_unusable_prompt() {
    let sandbox = Sandbox::new("prompts-validate").unwrap();
    let ingest = sandbox.config(&json!({})).unwrap();
    let triage = sandbox.triage(&json!({})).unwrap();
    assert_eq!(triage_validate(&ingest, &triage).unwrap(), Ok(()));
    fs::write(sandbox.prompts().unwrap().join("topic.md"), "x").unwrap();
    let err = triage_validate(&ingest, &triage).unwrap().unwrap_err();
    assert_eq!(err, "triage.prompts_dir: topic: unusable");
}
