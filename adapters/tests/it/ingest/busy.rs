use adapters::store::Archive;
use rusqlite::Connection;
use serde_json::json;

use crate::feeds::{event, feed};
use crate::support::{Sandbox, keys, run_logged};

const BRIEF: u64 = 50;
const SHORT: u64 = 2_000;

#[tokio::test]
async fn busy_store_backs_off_instead_of_failing() {
    let sandbox = Sandbox::new("ingest-busy").unwrap();
    drop(Archive::open(&sandbox.database(), BRIEF).unwrap());
    let holder = Connection::open(sandbox.database()).unwrap();
    holder.execute_batch("BEGIN IMMEDIATE").unwrap();
    sandbox.write("listen.txt", &feed(&[event("m1")])).unwrap();
    let config = sandbox.config(&json!({"busy_timeout_ms": BRIEF})).unwrap();
    let (code, logs) = run_logged(&config, SHORT).await.unwrap();
    holder.execute_batch("ROLLBACK").unwrap();
    assert_eq!(code, 0);
    assert!(logs.contains("store busy"));
    assert!(!logs.contains("store failed"));
    assert_eq!(keys(&sandbox.database()).unwrap(), Vec::<String>::new());
}
