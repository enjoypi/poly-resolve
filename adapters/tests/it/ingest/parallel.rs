use adapters::store::Archive;
use serde_json::json;

use crate::feeds::sealed_page;
use crate::support::{BUSY, NOON, Sandbox, keys, run_logged, sealed_envelope, texts};

const PAIR: u32 = 2;
const SHORT: u64 = 4_000;

fn calls(sandbox: &Sandbox) -> usize {
    sandbox
        .read("decrypt-calls.txt")
        .map_or(0, |text| text.lines().count())
}

#[tokio::test]
async fn page_decrypts_run_side_by_side() {
    let sandbox = Sandbox::new("ingest-parallel-page").unwrap();
    sandbox
        .write("pull.json", &sealed_page(&["m1", "m2", "m3"]))
        .unwrap();
    let config = sandbox.config(&json!({"dws": {"parallel": PAIR}})).unwrap();
    let (code, _logs) = run_logged(&config, SHORT).await.unwrap();
    assert_eq!(code, 0);
    assert_eq!(texts(&sandbox.database()).unwrap(), ["CLEARTEXT"; 3]);
    assert_eq!(calls(&sandbox), 3);
}

#[tokio::test]
async fn slow_page_decrypts_fill_the_window_then_stop() {
    let sandbox = Sandbox::new("ingest-parallel-slow").unwrap();
    sandbox
        .write("pull.json", &sealed_page(&["m1", "m2", "m3"]))
        .unwrap();
    let config = sandbox
        .config(&json!({"dws": {"env": {"FAKE_DECRYPT": "slow"}, "parallel": PAIR}}))
        .unwrap();
    let (code, _logs) = run_logged(&config, SHORT).await.unwrap();
    assert_eq!(code, 0);
    assert_eq!(calls(&sandbox), 2);
    assert_eq!(keys(&sandbox.database()).unwrap(), Vec::<String>::new());
}

#[tokio::test]
async fn stored_ciphertexts_heal_side_by_side() {
    let sandbox = Sandbox::new("ingest-parallel-heal").unwrap();
    let store = Archive::open(&sandbox.database(), BUSY).unwrap();
    for (key, millis) in [("k1", NOON), ("k2", NOON + 5)] {
        assert_eq!(store.save_event(&sealed_envelope(key, millis)).unwrap(), 1);
    }
    let config = sandbox
        .config(&json!({"dws": {"parallel": PAIR}, "heal_limit": PAIR}))
        .unwrap();
    let (code, logs) = run_logged(&config, SHORT).await.unwrap();
    assert_eq!(code, 0);
    assert_eq!(texts(&sandbox.database()).unwrap(), ["CLEARTEXT"; 2]);
    assert!(logs.contains("stored message decrypted"));
}
