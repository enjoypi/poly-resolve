use serde_json::{Value, from_str, json};

use super::rig::{
    BATCHES, KEY, LATER, LINES, NOTHING, PLACEMENTS, QUICK, SECOND, calls, decided, input_of,
    logged, questions_of, rule, seed, seed_from, start, table, two,
};
use crate::support::{Fallible, Sandbox};
use domain::review::Ruling;

const FLAGGED: &str =
    "SELECT key || '|' || state || '|' || flag || '|' || reason AS item FROM lines ORDER BY key";
const SECRET: &str = "abc-secret-9";

#[tokio::test]
async fn auto_mode_drops_flagged_lines_and_keeps_the_rest() {
    let sandbox = Sandbox::new("screen-auto").unwrap();
    seed(&sandbox, &two()).unwrap();
    sandbox
        .write(
            "screen.json",
            r#"{"lines":[{"line":"L1","risk":"high","spans":[],"topics":[]},{"line":"L2","risk":"low","spans":[],"topics":[]}]}"#,
        )
        .unwrap();
    start(&sandbox, &screened(&sandbox), QUICK).await.unwrap();
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
    assert_eq!(calls(&sandbox, "screen").unwrap(), 1);
    assert_eq!(calls(&sandbox, "decide").unwrap(), 1);
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
        assert!(logs.contains("screening failed"), "{name}");
        assert_eq!(calls(&sandbox, "decide").unwrap(), 0, "{name}");
        assert_eq!(table(&sandbox, BATCHES).unwrap(), NOTHING, "{name}");
        assert!(calls(&sandbox, "screen").unwrap() >= 1, "{name}");
    }
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

fn manual(sandbox: &Sandbox) -> Value {
    let mut patch = sandbox.screen("");
    if let Some(map) = patch.as_object_mut() {
        drop(map.insert("review".to_owned(), json!({"mode": "manual"})));
    }
    patch
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
            r#"{"lines":[{"line":"L1","risk":"high","spans":["abc-secret-9"],"topics":[]},{"line":"L2","risk":"high","spans":[],"topics":[]}]}"#,
        )
        .unwrap();
    let merged = manual(&sandbox);
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
    assert_eq!(calls(&sandbox, "decide").unwrap(), 0);
    for key in [KEY, SECOND] {
        let ruled = rule(&sandbox, &Ruling::approve(key)).unwrap();
        assert_eq!(ruled.as_deref(), Some("approved"));
    }
    start(&sandbox, &merged, QUICK).await.unwrap();
    let input = input_of(&decided(&sandbox).unwrap()).unwrap();
    assert!(!input.to_string().contains(SECRET));
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
    assert_eq!(calls(&sandbox, "screen").unwrap(), 1);
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
async fn without_a_screen_nothing_runs() {
    let sandbox = Sandbox::new("screen-off").unwrap();
    seed(&sandbox, &two()).unwrap();
    start(&sandbox, &json!({}), QUICK).await.unwrap();
    assert_eq!(calls(&sandbox, "screen").unwrap(), 0);
    assert_eq!(calls(&sandbox, "decide").unwrap(), 1);
}
