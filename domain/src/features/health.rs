use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Map, Value};

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "HealthCheckResponse")]
pub(crate) struct Response {
    status: &'static str,
}

pub(crate) fn check(_request: Map<String, Value>) -> Response {
    Response { status: "ok" }
}
