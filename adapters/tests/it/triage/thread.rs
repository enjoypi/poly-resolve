use serde_json::{Value, json};

use super::rig::{
    ALERTS, KEY, LATER, PLACEMENTS, QUICK, SECOND, SHORT, TOPICS, calls, home, one, seed,
    seed_from, start, table,
};
use crate::support::{Fallible, Sandbox};

const ALIASES: &str = "SELECT old || '|' || new AS item FROM topic_aliases ORDER BY old";

const BRIEF: u64 = 50;

#[tokio::test]
async fn abandoned_threads_start_fresh() {
    let sandbox = Sandbox::new("thread-aged").unwrap();
    seed(&sandbox, &one()).unwrap();
    start(&sandbox, &json!({}), SHORT).await.unwrap();
    sandbox.seed("UPDATE topics SET thread_at_ms = 0").unwrap();
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
async fn codex_that_cannot_start_or_finish_fails() {
    for (name, patch, reason) in [
        (
            "thread-spawn",
            json!({"codex": {"sandbox": {"bin": "/nonexistent/sandbox-exec"}}}),
            "io:NotFound",
        ),
        (
            "thread-timeout",
            json!({"codex": {"env": {"FAKE_CODEX": "slow"}, "timeout_ms": BRIEF}}),
            "io:TimedOut",
        ),
    ] {
        let sandbox = Sandbox::new(name).unwrap();
        seed(&sandbox, &one()).unwrap();
        start(&sandbox, &patch, QUICK).await.unwrap();
        assert_eq!(
            table(&sandbox, ALERTS).unwrap(),
            [format!("t1-0@1|failed|2||{reason}")],
            "{name}"
        );
    }
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
    assert_eq!(table(&sandbox, ALIASES).unwrap(), ["T1|T2", "t1-0|T2"]);
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
    assert!(prompt.contains("server down"));
    assert_eq!(table(&sandbox, TOPICS).unwrap(), ["T1|server d|T1"]);
    assert_eq!(table(&sandbox, ALIASES).unwrap(), ["t1-0|T1"]);
    assert_eq!(
        table(&sandbox, ALERTS).unwrap(),
        ["T1@2|sent|0|ANALYSIS|", "t1-0@1|sent|0|ANALYSIS|"]
    );
}

#[tokio::test]
async fn migrated_topics_leave_aliases_behind() {
    let sandbox = Sandbox::new("thread-migrate").unwrap();
    seed(&sandbox, &one()).unwrap();
    start(&sandbox, &quiet(), SHORT).await.unwrap();
    sandbox
        .seed("INSERT INTO topic_aliases VALUES ('t0-0', 't1-0'); UPDATE topics SET thread_id = 'T7' WHERE id = 't1-0';")
        .unwrap();
    start(&sandbox, &quiet(), SHORT).await.unwrap();
    assert_eq!(table(&sandbox, TOPICS).unwrap(), ["T7|server d|T7"]);
    assert_eq!(table(&sandbox, ALIASES).unwrap(), ["t0-0|T7", "t1-0|T7"]);
}

fn quiet() -> Value {
    json!({"stub": {"decide": {"p": "0.1"}}})
}

#[tokio::test]
async fn stale_topic_ids_resolve_to_the_renamed_thread() {
    let sandbox = Sandbox::new("thread-stale").unwrap();
    seed(&sandbox, &one()).unwrap();
    start(&sandbox, &quiet(), SHORT).await.unwrap();
    assert_eq!(table(&sandbox, TOPICS).unwrap(), ["t1-0|server d|"]);
    seed_from(&sandbox, &[(SECOND, "u1", "c1", "still down")], LATER).unwrap();
    sandbox
        .seed("CREATE TRIGGER refuse BEFORE INSERT ON members BEGIN SELECT RAISE(ABORT, 'refused'); END;")
        .unwrap();
    start(&sandbox, &quiet(), SHORT).await.unwrap();
    sandbox
        .seed("DROP TRIGGER refuse; INSERT INTO topics (id, label, probability, thread_id, until_ms, until_key, thread_at_ms, created_at_ms, last_at_ms) SELECT 'T1', label, probability, 'T1', until_ms, until_key, thread_at_ms, created_at_ms, last_at_ms FROM topics WHERE id = 't1-0'; UPDATE members SET topic_id = 'T1'; DELETE FROM topics WHERE id = 't1-0'; INSERT INTO topic_aliases VALUES ('t1-0', 'T1');")
        .unwrap();
    start(&sandbox, &quiet(), SHORT).await.unwrap();
    assert_eq!(table(&sandbox, TOPICS).unwrap(), ["T1|server d|T1"]);
    assert_eq!(
        table(&sandbox, PLACEMENTS).unwrap(),
        [format!("{KEY}|T1|0.1"), format!("{SECOND}|T1|0.1")]
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
