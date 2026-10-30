use domain::features::catalog::{Entry, Listing, build};
use serde_json::{Map, Value, json, to_string_pretty};

use crate::http::{SESSION_HEADER, USER_HEADER};

const SCHEMAS: &str = "#/components/schemas/";

fn fields<'listing>(entry: &Entry, listing: &'listing Listing) -> Option<&'listing Value> {
    let name = entry
        .params()
        .and_then(|reference| reference.get("$ref"))
        .and_then(Value::as_str)
        .and_then(|reference| reference.strip_prefix(SCHEMAS))
        .unwrap_or_default();
    listing.schemas().get(name)
}

fn header(name: &str) -> Value {
    json!({
        "description": "set by the gateway",
        "in": "header",
        "name": name,
        "required": false,
        "schema": {"type": "string"}
    })
}

fn operation(entry: &Entry, listing: &Listing) -> Value {
    let mut responses = Map::new();
    drop(responses.insert(
        "200".to_owned(),
        json!({
            "content": {"application/json": {"schema": entry.response()}},
            "description": "success"
        }),
    ));
    for failure in entry.failures() {
        drop(responses.insert(
            failure.status().to_string(),
            json!({
                "content": {"application/json": {"schema": listing.error()}},
                "description": failure.summary()
            }),
        ));
    }
    drop(responses.insert("403".to_owned(), json!({"description": "host not allowed"})));
    let mut operation = Map::new();
    drop(operation.insert(
        "parameters".to_owned(),
        Value::Array(parameters(entry, listing)),
    ));
    drop(entry.body().map(|body| {
        drop(responses.insert(
            "415".to_owned(),
            json!({"description": "content-type is not application/json"}),
        ));
        operation.insert(
            "requestBody".to_owned(),
            json!({
                "content": {"application/json": {"schema": body}},
                "required": true
            }),
        )
    }));
    drop(operation.insert("responses".to_owned(), Value::Object(responses)));
    Value::Object(operation)
}

fn parameters(entry: &Entry, listing: &Listing) -> Vec<Value> {
    let schema = fields(entry, listing);
    let required: Vec<&Value> = schema
        .and_then(|found| found.get("required"))
        .and_then(Value::as_array)
        .map_or_default(|names| names.iter().collect());
    let declared: Vec<Value> = schema
        .and_then(|found| found.get("properties"))
        .and_then(Value::as_object)
        .map_or_default(|properties| {
            properties
                .iter()
                .map(|(name, kind)| {
                    let place = if entry.path().contains(&format!("{{{name}}}")) {
                        "path"
                    } else {
                        "query"
                    };
                    json!({
                        "in": place,
                        "name": name,
                        "required": required.contains(&&Value::String(name.clone())),
                        "schema": kind
                    })
                })
                .collect()
        });
    [header(SESSION_HEADER), header(USER_HEADER)]
        .into_iter()
        .chain(declared)
        .collect()
}

pub(crate) fn render() -> String {
    let listing = build();
    let mut paths = Map::new();
    for entry in listing.entries() {
        let methods = paths
            .entry(entry.path())
            .or_insert_with(|| Value::Object(Map::new()));
        drop(
            methods
                .as_object_mut()
                .map(|known| known.insert(entry.method().to_owned(), operation(entry, &listing))),
        );
    }
    let document = json!({
        "components": {"schemas": listing.schemas()},
        "info": {"title": env!("CARGO_PKG_NAME"), "version": env!("CARGO_PKG_VERSION")},
        "openapi": "3.1.0",
        "paths": paths
    });
    to_string_pretty(&document).unwrap_or_default()
}
