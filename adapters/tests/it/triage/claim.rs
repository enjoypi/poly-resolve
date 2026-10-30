use serde_json::json;

use super::rig::{ALERTS, KEY, NOTHING, QUICK, SECOND, SHORT, calls, logged, one, seed, table};
use crate::support::Sandbox;

const ONE: u32 = 1;
const TWICE: usize = 2;

#[tokio::test]
async fn analysis_does_not_block_batches() {
    let sandbox = Sandbox::new("triage-claim-lanes").unwrap();
    seed(
        &sandbox,
        &[
            (KEY, "u1", "c1", "server down"),
            (SECOND, "u2", "c2", "db down"),
        ],
    )
    .unwrap();
    let patch = json!({
        "codex": {"env": {"FAKE_CODEX": "slow"}},
        "limits": {"batch": ONE},
    });
    let logs = logged(&sandbox, &patch, SHORT).await.unwrap();
    assert_eq!(calls(&sandbox, "decide").unwrap(), TWICE);
    assert_eq!(calls(&sandbox, "codex").unwrap(), 1);
    assert!(logs.contains("batch decided"));
}

#[tokio::test]
async fn analysis_is_not_rerun_when_its_result_cannot_be_stored() {
    let sandbox = Sandbox::new("triage-claim-advance").unwrap();
    seed(&sandbox, &one()).unwrap();
    sandbox
        .seed("CREATE TRIGGER refuse BEFORE UPDATE OF state ON alerts WHEN NEW.state = 'analyzed' BEGIN SELECT RAISE(ABORT, 'refused'); END;")
        .unwrap();
    let logs = logged(&sandbox, &json!({}), SHORT).await.unwrap();
    assert!(logs.contains("ledger write failed"));
    assert_eq!(calls(&sandbox, "codex").unwrap(), TWICE);
    assert_eq!(table(&sandbox, ALERTS).unwrap(), ["t1-0@1|pending|2||"]);
}

#[tokio::test]
async fn analysis_waits_when_the_claim_cannot_be_stored() {
    let sandbox = Sandbox::new("triage-claim-alert").unwrap();
    seed(&sandbox, &one()).unwrap();
    sandbox
        .seed("CREATE TRIGGER refuse BEFORE UPDATE OF attempts ON alerts WHEN NEW.attempts > OLD.attempts BEGIN SELECT RAISE(ABORT, 'refused'); END;")
        .unwrap();
    drop(logged(&sandbox, &json!({}), QUICK).await.unwrap());
    assert_eq!(calls(&sandbox, "codex").unwrap(), 0);
    assert_eq!(table(&sandbox, ALERTS).unwrap(), ["t1-0@1|pending|0||"]);
}

#[tokio::test]
async fn decision_is_not_resent_when_it_cannot_be_applied() {
    let sandbox = Sandbox::new("triage-claim-apply").unwrap();
    seed(&sandbox, &one()).unwrap();
    sandbox
        .seed("CREATE TRIGGER refuse BEFORE INSERT ON alerts BEGIN SELECT RAISE(ABORT, 'refused'); END;")
        .unwrap();
    let logs = logged(&sandbox, &json!({}), QUICK).await.unwrap();
    assert!(logs.contains("ledger transaction failed"));
    assert_eq!(calls(&sandbox, "decide").unwrap(), 1);
}

#[tokio::test]
async fn decision_waits_when_the_claim_cannot_be_stored() {
    let sandbox = Sandbox::new("triage-claim-batch").unwrap();
    seed(&sandbox, &one()).unwrap();
    sandbox
        .seed("CREATE TRIGGER refuse BEFORE UPDATE OF attempts ON batches BEGIN SELECT RAISE(ABORT, 'refused'); END;")
        .unwrap();
    drop(logged(&sandbox, &json!({}), QUICK).await.unwrap());
    assert_eq!(calls(&sandbox, "decide").unwrap(), 0);
    assert_eq!(table(&sandbox, ALERTS).unwrap(), NOTHING);
}

#[tokio::test]
async fn delivery_waits_when_the_claim_cannot_be_stored() {
    let sandbox = Sandbox::new("triage-claim-deliver").unwrap();
    seed(&sandbox, &one()).unwrap();
    sandbox
        .seed("CREATE TRIGGER refuse BEFORE UPDATE OF attempts ON alerts WHEN OLD.state = 'analyzed' AND NEW.attempts > OLD.attempts BEGIN SELECT RAISE(ABORT, 'refused'); END;")
        .unwrap();
    drop(logged(&sandbox, &json!({}), QUICK).await.unwrap());
    assert_eq!(calls(&sandbox, "codex").unwrap(), 1);
    assert_eq!(calls(&sandbox, "send").unwrap(), 0);
}
