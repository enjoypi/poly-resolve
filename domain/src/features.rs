pub mod catalog;
mod health;
mod ingest;
mod messages;

use std::time::Instant;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, from_str, to_string};

use crate::inbox::{Paging, Reader};
use crate::{AppError, Errors};

static ROUTES: [Route; 5] = [
    Route {
        action: "check",
        call: |request, _context| typed(request, health::check),
        failures: &[],
        feature: "health",
        schema: schemas::<Map<String, Value>, health::Response>,
        trace: Trace::Full,
    },
    Route {
        action: "parse_event",
        call: |request, _context| fallible(request, ingest::parse_event),
        failures: &[],
        feature: "ingest",
        schema: schemas::<ingest::ParseRequest, ingest::Envelope>,
        trace: Trace::Redacted,
    },
    Route {
        action: "parse_page",
        call: |request, _context| fallible(request, ingest::parse_page),
        failures: &[],
        feature: "ingest",
        schema: schemas::<ingest::ParseRequest, ingest::PageResponse>,
        trace: Trace::Redacted,
    },
    Route {
        action: "window",
        call: |request, _context| fallible(request, ingest::window),
        failures: &[],
        feature: "ingest",
        schema: schemas::<ingest::WindowRequest, ingest::WindowResponse>,
        trace: Trace::Full,
    },
    Route {
        action: "list",
        call: |request, context| queried(request, context, messages::list),
        failures: &[AppError::Storage],
        feature: "messages",
        schema: schemas::<messages::ListRequest, messages::ListResponse>,
        trace: Trace::Redacted,
    },
];

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

#[derive(Clone, Copy, Debug)]
pub struct Context<'ctx> {
    errors: &'ctx Errors,
    paging: &'ctx Paging,
    reader: &'ctx dyn Reader,
}

impl<'ctx> Context<'ctx> {
    const fn errors(&self) -> &Errors {
        self.errors
    }

    #[must_use = "上下文需交给 dispatch"]
    pub const fn new(errors: &'ctx Errors, paging: &'ctx Paging, reader: &'ctx dyn Reader) -> Self {
        Self {
            errors,
            paging,
            reader,
        }
    }

    const fn paging(&self) -> Paging {
        *self.paging
    }

    const fn reader(&self) -> &dyn Reader {
        self.reader
    }
}

type Describer = fn(&mut SchemaGenerator) -> (Schema, Schema);

type Fallible<Req, Resp> = fn(&Req) -> Result<Resp, AppError>;

type Handler = fn(&str, &Context<'_>) -> Result<String, AppError>;

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

type Queried<Req, Resp> = fn(&Req, &Context<'_>) -> Result<Resp, AppError>;

struct Route {
    action: &'static str,
    call: Handler,
    failures: &'static [AppError],
    feature: &'static str,
    schema: Describer,
    trace: Trace,
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

#[derive(Clone, Copy, Debug)]
enum Trace {
    Full,
    Redacted,
}

impl Trace {
    fn view(self, text: &str) -> String {
        match self {
            Self::Full => text.to_owned(),
            Self::Redacted => format!("redacted:{}", text.len()),
        }
    }
}

fn decode<Req>(request: &str) -> Result<Req, AppError>
where
    Req: DeserializeOwned,
{
    from_str(request).map_err(|_err| AppError::BadRequest)
}

pub fn dispatch(context: &Context<'_>, call: &Call<'_>) -> Result<String, AppError> {
    let errors = context.errors();
    let start = Instant::now();
    let found = route(call.target);
    let result = found.map_or(Err(AppError::UnknownAction), |hit| {
        (hit.call)(call.request, context)
    });
    let trace = found.map_or(Trace::Full, |hit| hit.trace);
    tracing::debug!(
        target: "dispatch",
        transport = call.origin.transport,
        session_id = call.origin.session_id.unwrap_or_default(),
        user = call.origin.user.unwrap_or_default(),
        feature = call.target.feature,
        action = call.target.action,
        request = trace.view(call.request),
        response = trace.view(&result.clone().unwrap_or_else(|err| err.body(errors))),
        ok = result.is_ok(),
        elapsed_us = u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX),
    );
    result
}

fn fallible<Req, Resp>(request: &str, handler: Fallible<Req, Resp>) -> Result<String, AppError>
where
    Req: DeserializeOwned,
    Resp: Serialize,
{
    let resp = handler(&decode(request)?)?;
    Ok(to_string(&resp).unwrap_or_default())
}

fn queried<Req, Resp>(
    request: &str,
    context: &Context<'_>,
    handler: Queried<Req, Resp>,
) -> Result<String, AppError>
where
    Req: DeserializeOwned,
    Resp: Serialize,
{
    let resp = handler(&decode(request)?, context)?;
    Ok(to_string(&resp).unwrap_or_default())
}

fn route(target: Target<'_>) -> Option<&'static Route> {
    ROUTES
        .iter()
        .find(|hit| hit.feature == target.feature && hit.action == target.action)
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
    decode(request).map(|req| to_string(&handler(req)).unwrap_or_default())
}
