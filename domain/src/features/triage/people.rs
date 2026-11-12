use alloc::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const MIN_SOURCES: usize = 2;

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriageContact")]
pub(crate) struct Contact {
    name: String,
    sender_id: String,
    source: String,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriageIdentity")]
pub(crate) struct Identity {
    name: String,
    person_id: String,
    sender_id: String,
    source: String,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "TriagePeopleRequest")]
pub(crate) struct PeopleRequest {
    senders: Vec<Contact>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "TriagePeopleResponse")]
pub(crate) struct PeopleResponse {
    identities: Vec<Identity>,
}

type Sources<'senders> = BTreeMap<&'senders str, BTreeSet<&'senders str>>;

fn normal(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

pub(crate) fn people(req: PeopleRequest) -> PeopleResponse {
    let PeopleRequest { senders } = req;
    let mut named: BTreeMap<String, Sources<'_>> = BTreeMap::new();
    for sender in &senders {
        let _: bool = named
            .entry(normal(&sender.name))
            .or_default()
            .entry(sender.source.as_str())
            .or_default()
            .insert(sender.sender_id.as_str());
    }
    let identities = senders
        .iter()
        .map(|sender| {
            let name = normal(&sender.name);
            let shared = named.get(&name).is_some_and(|sources| {
                !name.is_empty()
                    && sources.len() >= MIN_SOURCES
                    && sources.values().all(|ids| ids.len() == 1)
            });
            let person_id = if shared {
                format!("name:{name}")
            } else {
                format!("{}:{}", sender.source, sender.sender_id)
            };
            Identity {
                name,
                person_id,
                sender_id: sender.sender_id.clone(),
                source: sender.source.clone(),
            }
        })
        .collect();
    PeopleResponse { identities }
}
