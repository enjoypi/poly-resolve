use adapters::store::Archive;
use serde_json::json;

use super::rig::{QUICK, start, table};
use crate::support::{BUSY, Sandbox};

const LEGACY: &str = "INSERT INTO lines (key, conversation_id, occurred_at_ms, sender, text, state, reason, reviewer, flag, hints, rules_rev, batch_id, created_at_ms, updated_at_ms) VALUES ('a', 'c1', 1, '', '', 'skipped', 'own', '', '', '[]', 0, 0, 1, 1), ('b', 'c1', 2, 's', 't', 'skipped', 'expired', '', '', '[]', 0, 0, 1, 1), ('c', 'c1', 3, 's', 't', 'pending', '', 'me', '', '[]', 0, 4, 1, 1), ('d', 'c1', 4, 's', 't', 'leaked', '', '', '', '[]', 0, 4, 1, 1), ('e', 'c1', 5, '', '', 'skipped', 'sealed', '', '', '[]', 0, 0, 1, 1), ('f', 'c1', 6, '', '', 'skipped', 'empty', '', '', '[]', 0, 0, 1, 1), ('g', 'c1', 7, 's', 't', 'rejected', 'screen', '', 'unmasked', '[]', 0, 3, 1, 1), ('h', 'c1', 8, 's', 't', 'rejected', 'screen', '', 'unscreened', '[]', 0, 3, 1, 1), ('i', 'c1', 9, 's', 't', 'rejected', 'noise', '', '', '[]', 0, 3, 1, 1), ('j', 'c1', 10, '', '', 'skipped', 'notice', '', '', '[]', 0, 0, 1, 1);";
const STATES: &str = "SELECT key || '|' || state || '|' || reason || '|' || batch_id || '|' || reviewer AS item FROM lines ORDER BY key";

#[tokio::test]
async fn legacy_line_states_are_migrated_once() {
    let sandbox = Sandbox::new("migrate-lines").unwrap();
    drop(Archive::open(&sandbox.database(), BUSY).unwrap());
    sandbox.seed(LEGACY).unwrap();
    let off = json!({"mode": "disabled"});
    for _round in ["first", "second"] {
        start(&sandbox, &off, QUICK).await.unwrap();
        assert_eq!(
            table(&sandbox, STATES).unwrap(),
            [
                "c|queued||0|",
                "d|queued||0|",
                "e|withheld|sealed|0|",
                "f|withheld|empty|0|",
                "g|withheld|risky|3|",
                "h|withheld|unscreened|3|",
                "i|rejected|noise|3|",
                "j|skipped|notice|0|",
            ]
        );
    }
}
