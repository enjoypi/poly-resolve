use alloc::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const APART: &str = "apart";
const CLUE: &str = "clue";

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageCandidate")]
pub(crate) struct Candidate {
    from: String,
    into: String,
    keys: Vec<String>,
    kind: String,
    reason: String,
    size: usize,
}

impl Candidate {
    fn action(&self) -> Option<Kind> {
        if self.kind == APART {
            (!self.keys.is_empty() && self.keys.len() < self.size).then_some(Kind::Split)
        } else if self.kind == CLUE && self.keys.len().saturating_mul(2) < self.size {
            Some(Kind::Move)
        } else {
            Some(Kind::Merge)
        }
    }
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriageChange")]
pub(crate) struct Change {
    from: String,
    into: String,
    keys: Vec<String>,
    kind: Kind,
    reason: String,
}

#[derive(Clone, Copy, JsonSchema, Serialize)]
#[schemars(rename = "TriageChangeKind")]
#[serde(rename_all = "lowercase")]
pub(crate) enum Kind {
    Merge,
    Move,
    Split,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageRegroupRequest")]
pub(crate) struct RegroupRequest {
    candidates: Vec<Candidate>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriageRegroupResponse")]
pub(crate) struct RegroupResponse {
    changes: Vec<Change>,
}

pub(crate) fn regroup(req: RegroupRequest) -> RegroupResponse {
    let RegroupRequest { candidates } = req;
    let mut busy: BTreeSet<String> = BTreeSet::new();
    let mut changes = Vec::new();
    for candidate in candidates {
        let Some(kind) = candidate.action() else {
            continue;
        };
        let touched: Vec<String> = [&candidate.from, &candidate.into]
            .into_iter()
            .filter(|topic| !topic.is_empty())
            .cloned()
            .collect();
        if touched.iter().any(|topic| busy.contains(topic)) {
            continue;
        }
        busy.extend(touched);
        changes.push(Change {
            from: candidate.from,
            into: candidate.into,
            keys: match kind {
                Kind::Merge => Vec::new(),
                Kind::Move | Kind::Split => candidate.keys,
            },
            kind,
            reason: candidate.reason,
        });
    }
    RegroupResponse { changes }
}
