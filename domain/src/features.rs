pub mod catalog;
mod health;

use std::time::Instant;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, from_str, to_string};

use crate::{AppError, Errors};

static ROUTES: [Route; 1] = [Route {
    action: "check",
    call: |request| typed(request, health::check),
    feature: "health",
    schema: schemas::<Map<String, Value>, health::Response>,
}];

#[derive(Clone, Copy, Debug)]
pub struct Call<'call> {
    origin: Origin<'call>,
    request: &'call str,
    target: Target<'call>,
}

impl<'call> Call<'call> {
    #[must_use = "调用需交给 dispatch"]
    pub const fn new(origin: Origin<'call>, target: Target<'call>, request: &'call str) -> Self {
        Self {
            origin,
            request,
            target,
        }
    }
}

type Describer = fn(&mut SchemaGenerator) -> (Schema, Schema);

type Handler = fn(&str) -> Result<String, AppError>;

#[derive(Clone, Copy, Debug)]
pub struct Origin<'call> {
    session_id: Option<&'call str>,
    transport: &'call str,
    user: Option<&'call str>,
}

impl<'call> Origin<'call> {
    #[must_use = "来源需放入 Call"]
    pub const fn new(
        transport: &'call str,
        session_id: Option<&'call str>,
        user: Option<&'call str>,
    ) -> Self {
        Self {
            session_id,
            transport,
            user,
        }
    }
}

struct Route {
    action: &'static str,
    call: Handler,
    feature: &'static str,
    schema: Describer,
}

#[derive(Clone, Copy, Debug)]
pub struct Target<'call> {
    action: &'call str,
    feature: &'call str,
}

impl<'call> Target<'call> {
    #[must_use = "目标需放入 Call"]
    pub const fn new(feature: &'call str, action: &'call str) -> Self {
        Self { action, feature }
    }
}

pub fn dispatch(errors: &Errors, call: &Call<'_>) -> Result<String, AppError> {
    let start = Instant::now();
    let result = route(call.target, call.request);
    tracing::debug!(
        target: "dispatch",
        transport = call.origin.transport,
        session_id = call.origin.session_id.unwrap_or_default(),
        user = call.origin.user.unwrap_or_default(),
        feature = call.target.feature,
        action = call.target.action,
        request = call.request,
        response = result.clone().unwrap_or_else(|err| err.body(errors)),
        ok = result.is_ok(),
        elapsed_us = u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX),
    );
    result
}

fn route(target: Target<'_>, request: &str) -> Result<String, AppError> {
    ROUTES
        .iter()
        .find(|route| route.feature == target.feature && route.action == target.action)
        .map_or(Err(AppError::UnknownAction), |route| (route.call)(request))
}

fn schemas<Req, Resp>(generator: &mut SchemaGenerator) -> (Schema, Schema)
where
    Req: JsonSchema,
    Resp: JsonSchema,
{
    (
        generator.subschema_for::<Req>(),
        generator.subschema_for::<Resp>(),
    )
}

fn typed<Req, Resp>(request: &str, handler: fn(Req) -> Resp) -> Result<String, AppError>
where
    Req: DeserializeOwned,
    Resp: Serialize,
{
    from_str(request)
        .map(|req| to_string(&handler(req)).unwrap_or_default())
        .map_err(|_err| AppError::BadRequest)
}
