use schemars::JsonSchema;
use serde::Serialize;

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "HealthCheckResponse")]
pub(crate) struct Response {
    status: &'static str,
}

pub(crate) const fn check() -> Response {
    Response { status: "ok" }
}
