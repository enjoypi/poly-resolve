use serde_json::{Value, json};

use super::rig::{
    ALERTS, LATER, QUICK, SECOND, SHORT, TOPICS, calls, home, one, seed, seed_from, start, table,
};
use crate::support::{Fallible, Sandbox};

#[tokio::test]
async fn aged_threads_start_fresh() {
    let sandbox = Sandbox::new("thread-aged").unwrap();
    seed(&sandbox, &one()).unwrap();
    start(&sandbox, &json!({}), SHORT).await.unwrap();
    sandbox.seed("UPDATE topics SET thread_at_ms = 1").unwrap();
    seed_from(&sandbox, &[(SECOND, "u1", "c1", "still down")], LATER).unwrap();
    start(&sandbox, &json!({}), SHORT).await.unwrap();
    assert_eq!(sandbox.read("codex-args.txt").unwrap(), "codex -\n");
    let prompt = sandbox.read("codex-stdin.txt").unwrap();
    assert!(prompt.starts_with("ctx:") && prompt.contains("server down"));
    assert_eq!(calls(&sandbox, "codex").unwrap(), 2);
}

#[tokio::test]
async fn codex_args_expand_declared_variables() {
    let sandbox = Sandbox::new("thread-args").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"codex": {"args": ["codex", "url=${TRIAGE_URL}", "${UNDECLARED}"]}});
    start(&sandbox, &patch, SHORT).await.unwrap();
    assert_eq!(
        sandbox.read("codex-args.txt").unwrap(),
        format!("codex url={} ${{UNDECLARED}} -\n", home().unwrap())
    );
}

#[tokio::test]
async fn failed_resume_starts_a_new_thread() {
    let patch = json!({"codex": {"env": {"FAKE_CODEX": "resume_fail"}}});
    let sandbox = two_rounds("thread-drop", &patch).await.unwrap();
    assert_eq!(
        table(&sandbox, ALERTS).unwrap(),
        ["T1@2|sent|0|ANALYSIS|", "t1-0@1|sent|0|ANALYSIS|"]
    );
    assert_eq!(calls(&sandbox, "codex").unwrap(), 3);
    assert_eq!(sandbox.read("codex-args.txt").unwrap(), "codex -\n");
    assert_eq!(table(&sandbox, TOPICS).unwrap(), ["T2|server d|T2"]);
}

#[tokio::test]
async fn first_analysis_binds_the_thread_and_follow_ups_resume() {
    let sandbox = two_rounds("thread-resume", &json!({})).await.unwrap();
    assert_eq!(
        sandbox.read("codex-args.txt").unwrap(),
        "codex resume T1 -\n"
    );
    let prompt = sandbox.read("codex-stdin.txt").unwrap();
    assert!(prompt.starts_with("next:") && prompt.contains("still down"));
    assert!(!prompt.contains("server down"));
    assert_eq!(table(&sandbox, TOPICS).unwrap(), ["T1|server d|T1"]);
    assert_eq!(
        table(&sandbox, ALERTS).unwrap(),
        ["T1@2|sent|0|ANALYSIS|", "t1-0@1|sent|0|ANALYSIS|"]
    );
}

async fn two_rounds(name: &str, second: &Value) -> Fallible<Sandbox> {
    let sandbox = Sandbox::new(name)?;
    seed(&sandbox, &one())?;
    start(&sandbox, &json!({}), SHORT).await?;
    seed_from(&sandbox, &[(SECOND, "u1", "c1", "still down")], LATER)?;
    start(&sandbox, second, SHORT).await?;
    Ok(sandbox)
}

#[tokio::test]
async fn unusable_codex_output_is_a_failure() {
    let sandbox = Sandbox::new("thread-junk").unwrap();
    seed(&sandbox, &one()).unwrap();
    let patch = json!({"codex": {"env": {"FAKE_CODEX": "junk"}}});
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, ALERTS).unwrap(),
        ["t1-0@1|failed|2||exit:0"]
    );
    assert_eq!(table(&sandbox, TOPICS).unwrap(), ["t1-0|server d|"]);
}
