use serde_json::json;

use super::rig::{KEY, LINES, PLACEMENTS, QUICK, SECOND, SHORT, THIRD, home, seed, start, table};
use crate::support::{SEALED, Sandbox};

const REVEAL: &str = "UPDATE messages SET text = 'clear now' WHERE key = 'dingtalk:acct:m2';";

#[tokio::test]
async fn revealed_text_is_lined_and_placed_again() {
    let sandbox = Sandbox::new("withheld-revealed").unwrap();
    let rows = [
        (KEY, "u1", "c1", "server down"),
        (SECOND, "u1", "c1", SEALED),
    ];
    seed(&sandbox, &rows).unwrap();
    let quiet = json!({"stub": {"decide": {"p": "0.1"}}});
    start(&sandbox, &quiet, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, PLACEMENTS).unwrap(),
        [format!("{KEY}|t1-0|0.1"), format!("{SECOND}|t1-0|")]
    );
    sandbox.seed(REVEAL).unwrap();
    assert_eq!(
        table(&sandbox, PLACEMENTS).unwrap(),
        [format!("{KEY}|t1-0|0.1")]
    );
    start(&sandbox, &quiet, QUICK).await.unwrap();
    assert_eq!(
        table(&sandbox, LINES).unwrap(),
        [
            format!("{KEY}|approved||1"),
            format!("{SECOND}|approved||2")
        ]
    );
    assert_eq!(
        table(&sandbox, PLACEMENTS).unwrap(),
        [format!("{KEY}|t1-0|0.1"), format!("{SECOND}|t1-0|0.1")]
    );
}

#[tokio::test]
async fn withheld_neighbours_reach_codex_as_a_note() {
    let sandbox = Sandbox::new("withheld-note").unwrap();
    let own = home().unwrap();
    let rows = [
        (KEY, "u1", "c1", "server down"),
        (SECOND, "u1", "c1", SEALED),
        (THIRD, own.as_str(), "c1", "on it"),
    ];
    seed(&sandbox, &rows).unwrap();
    start(&sandbox, &json!({}), SHORT).await.unwrap();
    assert_eq!(
        table(&sandbox, LINES).unwrap(),
        [
            format!("{KEY}|approved||1"),
            format!("{SECOND}|withheld|sealed|0"),
            format!("{THIRD}|approved||1"),
        ]
    );
    assert_eq!(
        table(&sandbox, PLACEMENTS).unwrap(),
        [
            format!("{KEY}|T1|0.95"),
            format!("{SECOND}|T1|"),
            format!("{THIRD}|T1|"),
        ]
    );
    let prompt = sandbox.read("codex-stdin.txt").unwrap();
    assert!(prompt.contains(r#"{"note":"1 held: sealed"}"#), "{prompt}");
    assert!(prompt.contains(r#""role":"own""#), "{prompt}");
    assert!(!prompt.contains(SEALED));
}
