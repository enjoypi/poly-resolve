use serde_json::{Value, from_str, json};

use super::rig::{
    BATCHES, KEY, LATER, LINES, NOTHING, PLACEMENTS, QUICK, SECOND, THIRD, calls, decided, draw,
    input_of, logged, questions_of, rule, seed, seed_from, start, table, two,
};
use crate::support::{Fallible, Sandbox};
use domain::review::{Draw, Ruling};

const BRIEF_RETRY: u64 = 300;
const CAPPED_RETRY: u64 = 400;
const FLAGGED: &str =
    "SELECT key || '|' || state || '|' || flag || '|' || reason AS item FROM lines ORDER BY key";
const MANY_CALLS: usize = 24;
const MISSES: &str =
    "SELECT kind || '|' || key || '|' || text || '|' || spans AS item FROM misses ORDER BY key";
const SECRET: &str = "abc-secret-9";
const TEN: u32 = 10;

fn aimed(sandbox: &Sandbox, mode: &str, only: &str) -> Value {
    let mut patch = sandbox.screen("");
    if let Some(map) = patch.as_object_mut() {
        drop(map.insert(
            "stub".to_owned(),
            json!({"screen": {"mode": mode, "only": only}}),
        ));
    }
    patch
}

#[tokio::test]
async fn auto_mode_drops_flagged_lines_and_keeps_the_rest() {
    let sandbox = Sandbox::new("screen-auto").unwrap();
    seed(&sandbox, &two()).unwrap();
    sandbox
        .write(
            "screen.json",
            r#"{"lines":[{"line":"L1","risk":"high","spans":[],"topics":[]}]}"#,
        )
        .unwrap();
    start(&sandbox, &aimed(&sandbox, "", "server down"), QUICK)
        .await
        .unwrap();
    assert_eq!(
        table(&sandbox, MISSES).unwrap(),
        [format!("unmasked|{KEY}|server down|[]")]
    );
    assert_eq!(
        table(&sandbox, FLAGGED).unwrap(),
        [
            format!("{KEY}|rejected|unmasked|screen"),
            format!("{SECOND}|approved||")
        ]
    );
    assert_eq!(questions_of(&decided(&sandbox).unwrap()), ["urgent_1"]);
    assert_eq!(
        table(&sandbox, PLACEMENTS).unwrap(),
        [format!("{SECOND}|T1|0.95")]
    );
}

#[tokio::test]
async fn clean_lines_pass_through_to_the_decision() {
    let sandbox = Sandbox::new("screen-clean").unwrap();
    seed(&sandbox, &two()).unwrap();
    start(&sandbox, &screened(&sandbox), QUICK).await.unwrap();
    assert_eq!(calls(&sandbox, "screen").unwrap(), 2);
    assert_eq!(calls(&sandbox, "decide").unwrap(), 1);
    assert_eq!(table(&sandbox, MISSES).unwrap(), NOTHING);
    assert_eq!(
        table(&sandbox, LINES).unwrap(),
        [
            format!("{KEY}|approved||1"),
            format!("{SECOND}|approved||1")
        ]
    );
    let prompt: Value = from_str(&sandbox.read("screen-body.txt").unwrap()).unwrap();
    assert_eq!(prompt.get("model"), Some(&json!("local")));
    assert!(!sandbox.read("screen-body.txt").unwrap().contains(KEY));
}

#[tokio::test]
async fn failed_screening_is_retried_and_never_lets_lines_through() {
    for (name, mode) in [("fail", "fail"), ("junk", "junk")] {
        let sandbox = Sandbox::new(&format!("screen-{name}")).unwrap();
        seed(&sandbox, &two()).unwrap();
        let patch = sandbox.screen(mode);
        let logs = logged(&sandbox, &patch, QUICK).await.unwrap();
        assert!(logs.contains("line screening failed"), "{name}");
        assert!(logs.contains("\"retry_in_ms\":0"), "{name}");
        assert_eq!(calls(&sandbox, "decide").unwrap(), 0, "{name}");
        assert_eq!(table(&sandbox, BATCHES).unwrap(), NOTHING, "{name}");
        assert!(calls(&sandbox, "screen").unwrap() >= 1, "{name}");
        assert_eq!(
            logs.matches("no usable answers").count(),
            usize::from(mode == "junk"),
            "{name}"
        );
        assert_eq!(
            table(&sandbox, FLAGGED).unwrap(),
            [
                format!("{KEY}|screening||deferred"),
                format!("{SECOND}|screening||deferred")
            ],
            "{name}"
        );
    }
}

#[tokio::test]
async fn failed_screening_waits_longer_between_retries() {
    let sandbox = Sandbox::new("screen-backoff").unwrap();
    seed(&sandbox, &two()).unwrap();
    let mut patch = sandbox.screen("fail");
    if let Some(map) = patch.as_object_mut() {
        drop(map.insert(
            "limits".to_owned(),
            json!({"retry_after_ms": BRIEF_RETRY, "retry_max_ms": CAPPED_RETRY}),
        ));
    }
    let logs = logged(&sandbox, &patch, QUICK).await.unwrap();
    assert!(logs.contains("\"attempt\":1") && logs.contains("\"retry_in_ms\":300"));
    assert!(logs.contains("\"attempt\":3") && logs.contains("\"retry_in_ms\":400"));
    assert!(!logs.contains("\"retry_in_ms\":600"));
    let sent = calls(&sandbox, "screen").unwrap();
    assert!((4..MANY_CALLS).contains(&sent), "{sent}");
}

#[tokio::test]
async fn failing_line_does_not_hold_back_the_others() {
    let sandbox = Sandbox::new("screen-one-fails").unwrap();
    seed(&sandbox, &two()).unwrap();
    start(&sandbox, &aimed(&sandbox, "fail", "still down"), QUICK)
        .await
        .unwrap();
    assert_eq!(
        table(&sandbox, FLAGGED).unwrap(),
        [
            format!("{KEY}|approved||"),
            format!("{SECOND}|screening||deferred")
        ]
    );
    assert_eq!(calls(&sandbox, "decide").unwrap(), 0);
    assert_eq!(table(&sandbox, BATCHES).unwrap(), NOTHING);
}

#[tokio::test]
async fn hints_follow_renamed_topics() {
    let sandbox = Sandbox::new("screen-hints-renamed").unwrap();
    seed(&sandbox, &[(KEY, "u1", "c1", "server down")]).unwrap();
    start(&sandbox, &json!({"stub": {"decide": {"p": "0.1"}}}), QUICK)
        .await
        .unwrap();
    seed_from(&sandbox, &[(SECOND, "u1", "c2", "still down")], LATER).unwrap();
    sandbox
        .write(
            "screen.json",
            r#"{"lines":[{"line":"L1","risk":"low","spans":[],"topics":["t1-0"]}]}"#,
        )
        .unwrap();
    sandbox
        .seed("CREATE TRIGGER refuse BEFORE INSERT ON batches BEGIN SELECT RAISE(ABORT, 'refused'); END;")
        .unwrap();
    start(&sandbox, &screened(&sandbox), QUICK).await.unwrap();
    sandbox
        .seed("DROP TRIGGER refuse; INSERT INTO topics SELECT 'T1', label, probability, 'T1', until_ms, until_key, thread_at_ms, created_at_ms, last_at_ms FROM topics WHERE id = 't1-0'; UPDATE placements SET topic_id = 'T1'; DELETE FROM topics WHERE id = 't1-0'; INSERT INTO topic_aliases VALUES ('t1-0', 'T1'); INSERT INTO topics VALUES ('t9-0', 'other', 0.1, '', 0, '', 0, 0, 9999999999999);")
        .unwrap();
    let mut patch = screened(&sandbox);
    if let Some(map) = patch.as_object_mut() {
        drop(map.insert(
            "stub".to_owned(),
            json!({"decide": {"p": "0.1", "topics": "T1"}}),
        ));
    }
    start(&sandbox, &patch, QUICK).await.unwrap();
    let body = decided(&sandbox).unwrap();
    assert_eq!(
        body.pointer("/questions/1/choices/0/value"),
        Some(&json!("T1"))
    );
    assert_eq!(
        body.pointer("/questions/1/choices")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}

#[tokio::test]
async fn hints_narrow_the_topic_options() {
    let sandbox = Sandbox::new("screen-hints").unwrap();
    seed(&sandbox, &[(KEY, "u1", "c1", "server down")]).unwrap();
    start(&sandbox, &json!({"stub": {"decide": {"p": "0.1"}}}), QUICK)
        .await
        .unwrap();
    seed_next(&sandbox).unwrap();
    sandbox
        .write(
            "screen.json",
            r#"{"lines":[{"line":"L1","risk":"low","spans":[],"topics":["t1-0"]}]}"#,
        )
        .unwrap();
    let mut patch = screened(&sandbox);
    if let Some(map) = patch.as_object_mut() {
        drop(map.insert(
            "stub".to_owned(),
            json!({"decide": {"p": "0.1", "topics": "t1-0"}}),
        ));
    }
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, PLACEMENTS).unwrap(),
        [format!("{KEY}|t1-0|0.1"), format!("{SECOND}|t1-0|0.1")]
    );
    assert_eq!(calls(&sandbox, "decide").unwrap(), 2);
}

#[tokio::test]
async fn landed_lines_survive_shutdown() {
    let sandbox = Sandbox::new("screen-landed").unwrap();
    seed(&sandbox, &two()).unwrap();
    start(&sandbox, &aimed(&sandbox, "slow", "still down"), QUICK)
        .await
        .unwrap();
    assert_eq!(
        table(&sandbox, FLAGGED).unwrap(),
        [format!("{KEY}|approved||"), format!("{SECOND}|screening||")]
    );
}

fn manual(sandbox: &Sandbox) -> Value {
    let mut patch = sandbox.screen("");
    if let Some(map) = patch.as_object_mut() {
        drop(map.insert("review".to_owned(), json!({"mode": "manual"})));
    }
    patch
}

#[tokio::test]
async fn misnamed_answers_are_set_aside_after_retries() {
    let sandbox = Sandbox::new("screen-misnamed").unwrap();
    seed(&sandbox, &two()).unwrap();
    sandbox
        .write(
            "screen.json",
            r#"{"lines":[{"line":"L9","risk":"low","spans":[],"topics":[]}]}"#,
        )
        .unwrap();
    let logs = logged(&sandbox, &screened(&sandbox), QUICK).await.unwrap();
    assert!(logs.contains("bad_answer"));
    assert!(logs.contains("line screening abandoned"));
    assert_eq!(
        table(&sandbox, FLAGGED).unwrap(),
        [
            format!("{KEY}|rejected|unscreened|screen"),
            format!("{SECOND}|rejected|unscreened|screen")
        ]
    );
    assert_eq!(
        table(&sandbox, MISSES).unwrap(),
        [
            format!("unscreened|{KEY}|server down|[]"),
            format!("unscreened|{SECOND}|still down|[]")
        ]
    );
    assert_eq!(table(&sandbox, BATCHES).unwrap(), ["1|sent|0|"]);
}

#[tokio::test]
async fn missed_samples_that_cannot_be_stored_keep_the_line_unscreened() {
    let sandbox = Sandbox::new("screen-sample-fails").unwrap();
    let leaky = format!("token {SECRET} here");
    seed(&sandbox, &[(KEY, "u1", "c1", leaky.as_str())]).unwrap();
    sandbox
        .write(
            "screen.json",
            r#"{"lines":[{"line":"L1","risk":"high","spans":["abc-secret-9"],"topics":[]}]}"#,
        )
        .unwrap();
    start(&sandbox, &json!({"mode": "disabled"}), QUICK)
        .await
        .unwrap();
    sandbox
        .seed("CREATE TRIGGER refuse BEFORE INSERT ON misses BEGIN SELECT RAISE(ABORT, 'refused'); END;")
        .unwrap();
    let logs = logged(&sandbox, &screened(&sandbox), QUICK).await.unwrap();
    assert!(logs.contains("ledger transaction failed"));
    assert!(logs.contains("lane backing off"));
    assert_eq!(
        table(&sandbox, FLAGGED).unwrap(),
        [format!("{KEY}|screening||")]
    );
    assert_eq!(table(&sandbox, MISSES).unwrap(), NOTHING);
}

#[tokio::test]
async fn model_masks_spans_and_flags_unmaskable_secrets_for_review() {
    let sandbox = Sandbox::new("screen-flag").unwrap();
    let leaky = format!("token {SECRET} here");
    let rows = [
        (KEY, "u1", "c1", leaky.as_str()),
        (SECOND, "u1", "c1", "odd text"),
    ];
    seed(&sandbox, &rows).unwrap();
    sandbox
        .write(
            "screen.json",
            r#"{"lines":[{"line":"L1","risk":"high","spans":["abc-secret-9"],"topics":[]}]}"#,
        )
        .unwrap();
    let merged = manual(&sandbox);
    start(&sandbox, &merged, QUICK).await.unwrap();
    assert_eq!(draw(&sandbox, Draw::screened(TEN)).unwrap(), "1|2");
    assert_eq!(
        table(&sandbox, FLAGGED).unwrap(),
        [
            format!("{KEY}|screening||"),
            format!("{SECOND}|screening||")
        ]
    );
    start(&sandbox, &merged, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, FLAGGED).unwrap(),
        [
            format!("{KEY}|pending||"),
            format!("{SECOND}|pending|unmasked|")
        ]
    );
    assert_eq!(
        table(&sandbox, "SELECT text AS item FROM lines ORDER BY key").unwrap(),
        ["token [s] here", "odd text"]
    );
    assert_eq!(
        table(&sandbox, MISSES).unwrap(),
        [
            format!("rules|{KEY}|{leaky}|[\"{SECRET}\"]"),
            format!("unmasked|{SECOND}|odd text|[]")
        ]
    );
    assert_eq!(calls(&sandbox, "decide").unwrap(), 0);
    for key in [KEY, SECOND] {
        let ruled = rule(&sandbox, &Ruling::approve(key)).unwrap();
        assert_eq!(ruled.as_deref(), Some("approved"));
    }
    start(&sandbox, &merged, QUICK).await.unwrap();
    let input = input_of(&decided(&sandbox).unwrap()).unwrap();
    assert!(!input.to_string().contains(SECRET));
}

#[tokio::test]
async fn probe_interrupted_by_shutdown_changes_nothing() {
    let sandbox = Sandbox::new("screen-probe-slow").unwrap();
    seed(&sandbox, &two()).unwrap();
    let mut patch = sandbox.screen("");
    if let Some(map) = patch.as_object_mut() {
        drop(map.insert(
            "stub".to_owned(),
            json!({"screen": {"canary": "slow", "mode": "junk"}}),
        ));
    }
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(calls(&sandbox, "screen").unwrap(), 3);
    assert_eq!(
        table(&sandbox, FLAGGED).unwrap(),
        [
            format!("{KEY}|screening||"),
            format!("{SECOND}|screening||")
        ]
    );
}

fn screened(sandbox: &Sandbox) -> Value {
    sandbox.screen("")
}

fn seed_next(sandbox: &Sandbox) -> Fallible<()> {
    seed_from(sandbox, &[(SECOND, "u1", "c2", "still down")], LATER)
}

#[tokio::test]
async fn slow_screening_is_interrupted_by_shutdown() {
    let sandbox = Sandbox::new("screen-slow").unwrap();
    seed(&sandbox, &two()).unwrap();
    let patch = sandbox.screen("slow");
    start(&sandbox, &patch, QUICK).await.unwrap();
    assert_eq!(calls(&sandbox, "screen").unwrap(), 2);
    assert_eq!(calls(&sandbox, "decide").unwrap(), 0);
    assert_eq!(table(&sandbox, BATCHES).unwrap(), NOTHING);
    assert_eq!(
        table(&sandbox, LINES).unwrap(),
        [
            format!("{KEY}|screening||1"),
            format!("{SECOND}|screening||1")
        ]
    );
}

#[tokio::test]
async fn slow_screening_runs_at_most_parallel_requests() {
    let sandbox = Sandbox::new("screen-parallel").unwrap();
    let rows = [
        (KEY, "u1", "c1", "server down"),
        (SECOND, "u1", "c1", "still down"),
        (THIRD, "u1", "c1", "again"),
    ];
    seed(&sandbox, &rows).unwrap();
    start(&sandbox, &sandbox.screen("slow"), QUICK)
        .await
        .unwrap();
    assert_eq!(calls(&sandbox, "screen").unwrap(), 2);
    assert_eq!(
        table(&sandbox, LINES).unwrap(),
        [
            format!("{KEY}|screening||1"),
            format!("{SECOND}|screening||1"),
            format!("{THIRD}|screening||1")
        ]
    );
}

#[tokio::test]
async fn unanswerable_line_is_set_aside_while_the_rest_goes_on() {
    let sandbox = Sandbox::new("screen-unanswerable").unwrap();
    seed(&sandbox, &two()).unwrap();
    let logs = logged(&sandbox, &aimed(&sandbox, "junk", "still down"), QUICK)
        .await
        .unwrap();
    assert!(!logs.contains("no usable answers"));
    assert_eq!(
        table(&sandbox, FLAGGED).unwrap(),
        [
            format!("{KEY}|approved||"),
            format!("{SECOND}|rejected|unscreened|screen")
        ]
    );
    assert_eq!(calls(&sandbox, "decide").unwrap(), 1);
}

#[tokio::test]
async fn without_a_screen_nothing_runs() {
    let sandbox = Sandbox::new("screen-off").unwrap();
    seed(&sandbox, &two()).unwrap();
    start(&sandbox, &json!({}), QUICK).await.unwrap();
    assert_eq!(calls(&sandbox, "screen").unwrap(), 0);
    assert_eq!(calls(&sandbox, "decide").unwrap(), 1);
}
