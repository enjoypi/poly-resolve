pub mod catalog;
mod health;
mod ingest;
mod messages;
mod review;
mod triage;

use std::time::Instant;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{from_str, to_string};
use serde_urlencoded::from_str as from_query;

use crate::inbox::{Paging, Reader};
use crate::review::{Docket, Reason};
use crate::{AppError, Errors};

static ROUTES: [Route; 19] = [
    Route {
        action: "check",
        call: |_call, _context| Ok(to_string(&health::check()).unwrap_or_default()),
        failures: &[],
        feature: "health",
        http: Http::Bound("get", "/api/health", bare::<health::Response>),
        trace: Trace::Full,
    },
    Route {
        action: "assemble",
        call: |call, _context| typed(call.request, ingest::assemble),
        failures: &[],
        feature: "ingest",
        http: Http::Hidden,
        trace: Trace::Redacted,
    },
    Route {
        action: "cipher",
        call: |call, _context| typed(call.request, ingest::cipher),
        failures: &[],
        feature: "ingest",
        http: Http::Hidden,
        trace: Trace::Redacted,
    },
    Route {
        action: "parse_event",
        call: |call, _context| fallible(call.request, ingest::parse_event),
        failures: &[],
        feature: "ingest",
        http: Http::Hidden,
        trace: Trace::Redacted,
    },
    Route {
        action: "parse_page",
        call: |call, _context| fallible(call.request, ingest::parse_page),
        failures: &[],
        feature: "ingest",
        http: Http::Hidden,
        trace: Trace::Redacted,
    },
    Route {
        action: "reveal",
        call: |call, _context| typed(call.request, ingest::reveal),
        failures: &[],
        feature: "ingest",
        http: Http::Hidden,
        trace: Trace::Redacted,
    },
    Route {
        action: "window",
        call: |call, _context| fallible(call.request, ingest::window),
        failures: &[],
        feature: "ingest",
        http: Http::Hidden,
        trace: Trace::Full,
    },
    Route {
        action: "list",
        call: |call, context| listed(call.params, context, messages::list),
        failures: &[AppError::BadRequest, AppError::Storage],
        feature: "messages",
        http: Http::Bound(
            "get",
            "/api/messages",
            asked::<messages::ListRequest, messages::ListResponse>,
        ),
        trace: Trace::Redacted,
    },
    Route {
        action: "list",
        call: |call, context| listed(call.params, context, review::list),
        failures: &[AppError::BadRequest, AppError::Storage],
        feature: "review",
        http: Http::Bound(
            "get",
            "/api/reviews",
            asked::<review::ListRequest, review::ListResponse>,
        ),
        trace: Trace::Redacted,
    },
    Route {
        action: "update",
        call: |call, context| keyed(call, context, review::update),
        failures: &[
            AppError::BadRequest,
            AppError::NotFound,
            AppError::Conflict,
            AppError::Unprocessable,
            AppError::Storage,
        ],
        feature: "review",
        http: Http::Bound(
            "patch",
            "/api/reviews/{key}",
            changed::<review::Key, review::Update, review::RulingResponse>,
        ),
        trace: Trace::Redacted,
    },
    Route {
        action: "brief",
        call: |call, _context| fallible(call.request, triage::prompt::brief),
        failures: &[],
        feature: "triage",
        http: Http::Hidden,
        trace: Trace::Redacted,
    },
    Route {
        action: "line",
        call: |call, _context| fallible(call.request, triage::line::line),
        failures: &[],
        feature: "triage",
        http: Http::Hidden,
        trace: Trace::Redacted,
    },
    Route {
        action: "notice",
        call: |call, _context| typed(call.request, triage::prompt::notice),
        failures: &[],
        feature: "triage",
        http: Http::Hidden,
        trace: Trace::Redacted,
    },
    Route {
        action: "pack",
        call: |call, _context| fallible(call.request, triage::pack::pack),
        failures: &[],
        feature: "triage",
        http: Http::Hidden,
        trace: Trace::Redacted,
    },
    Route {
        action: "redact",
        call: |call, _context| fallible(call.request, triage::redact::redact),
        failures: &[],
        feature: "triage",
        http: Http::Hidden,
        trace: Trace::Redacted,
    },
    Route {
        action: "reply",
        call: |call, _context| fallible(call.request, triage::reply::reply),
        failures: &[],
        feature: "triage",
        http: Http::Hidden,
        trace: Trace::Redacted,
    },
    Route {
        action: "screen",
        call: |call, _context| typed(call.request, triage::screen::screen),
        failures: &[],
        feature: "triage",
        http: Http::Hidden,
        trace: Trace::Redacted,
    },
    Route {
        action: "screened",
        call: |call, _context| fallible(call.request, triage::screened::screened),
        failures: &[],
        feature: "triage",
        http: Http::Hidden,
        trace: Trace::Redacted,
    },
    Route {
        action: "sort",
        call: |call, _context| fallible(call.request, triage::sort::sort),
        failures: &[],
        feature: "triage",
        http: Http::Hidden,
        trace: Trace::Redacted,
    },
];

type Binding = (&'static str, &'static str, Describer);

#[derive(Clone, Copy, Debug)]
pub struct Call<'call> {
    origin: Origin<'call>,
    params: &'call str,
    request: &'call str,
    target: Target<'call>,
}

impl<'call> Call<'call> {
    #[must_use = "调用需交给 dispatch"]
    pub const fn new(origin: Origin<'call>, target: Target<'call>, request: &'call str) -> Self {
        Self {
            origin,
            params: "",
            request,
            target,
        }
    }

    #[must_use = "调用需交给 dispatch"]
    pub const fn with_params(self, params: &'call str) -> Self {
        Self { params, ..self }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Context<'ctx> {
    errors: &'ctx Errors,
    ports: Ports<'ctx>,
    rules: Rules<'ctx>,
}

impl<'ctx> Context<'ctx> {
    const fn docket(&self) -> &dyn Docket {
        self.ports.docket
    }

    const fn errors(&self) -> &Errors {
        self.errors
    }

    const fn learn_min_chars(&self) -> u32 {
        self.rules.learn_min_chars
    }

    #[must_use = "上下文需交给 dispatch"]
    pub const fn new(errors: &'ctx Errors, rules: Rules<'ctx>, ports: Ports<'ctx>) -> Self {
        Self {
            errors,
            ports,
            rules,
        }
    }

    const fn paging(&self) -> Paging {
        *self.rules.paging
    }

    const fn reader(&self) -> &dyn Reader {
        self.ports.reader
    }

    const fn reasons(&self) -> &[Reason] {
        self.rules.reasons
    }
}

type Describer = fn(&mut SchemaGenerator) -> Shapes;

type Fallible<Req, Resp> = fn(&Req) -> Result<Resp, AppError>;

type Handler = fn(&Call<'_>, &Context<'_>) -> Result<String, AppError>;

#[derive(Clone, Copy)]
enum Http {
    Bound(&'static str, &'static str, Describer),
    Hidden,
}

impl Http {
    const fn binding(self) -> Option<Binding> {
        match self {
            Self::Bound(method, path, describe) => Some((method, path, describe)),
            Self::Hidden => None,
        }
    }
}

type Keyed<Params, Req, Resp> = fn(&Params, Req, &Context<'_>) -> Result<Resp, AppError>;

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

#[derive(Clone, Copy, Debug)]
pub struct Ports<'ports> {
    docket: &'ports dyn Docket,
    reader: &'ports dyn Reader,
}

impl<'ports> Ports<'ports> {
    #[must_use = "端口需交给 Context"]
    pub const fn new(reader: &'ports dyn Reader, docket: &'ports dyn Docket) -> Self {
        Self { docket, reader }
    }
}

type Queried<Req, Resp> = fn(&Req, &Context<'_>) -> Result<Resp, AppError>;

struct Route {
    action: &'static str,
    call: Handler,
    failures: &'static [AppError],
    feature: &'static str,
    http: Http,
    trace: Trace,
}

#[derive(Clone, Copy, Debug)]
pub struct Rules<'rules> {
    learn_min_chars: u32,
    paging: &'rules Paging,
    reasons: &'rules [Reason],
}

impl<'rules> Rules<'rules> {
    #[must_use = "规则需交给 Context"]
    pub const fn new(
        paging: &'rules Paging,
        reasons: &'rules [Reason],
        learn_min_chars: u32,
    ) -> Self {
        Self {
            learn_min_chars,
            paging,
            reasons,
        }
    }
}

type Shapes = (Option<Schema>, Option<Schema>, Schema);

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

fn asked<Params, Resp>(generator: &mut SchemaGenerator) -> Shapes
where
    Params: JsonSchema,
    Resp: JsonSchema,
{
    (
        Some(generator.subschema_for::<Params>()),
        None,
        generator.subschema_for::<Resp>(),
    )
}

fn bare<Resp>(generator: &mut SchemaGenerator) -> Shapes
where
    Resp: JsonSchema,
{
    (None, None, generator.subschema_for::<Resp>())
}

fn changed<Params, Req, Resp>(generator: &mut SchemaGenerator) -> Shapes
where
    Params: JsonSchema,
    Req: JsonSchema,
    Resp: JsonSchema,
{
    (
        Some(generator.subschema_for::<Params>()),
        Some(generator.subschema_for::<Req>()),
        generator.subschema_for::<Resp>(),
    )
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
        (hit.call)(call, context)
    });
    let trace = found.map_or(Trace::Full, |hit| hit.trace);
    tracing::debug!(
        target: "dispatch",
        transport = call.origin.transport,
        session_id = call.origin.session_id.unwrap_or_default(),
        user = call.origin.user.unwrap_or_default(),
        feature = call.target.feature,
        action = call.target.action,
        params = trace.view(call.params),
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

fn keyed<Params, Req, Resp>(
    call: &Call<'_>,
    context: &Context<'_>,
    handler: Keyed<Params, Req, Resp>,
) -> Result<String, AppError>
where
    Params: DeserializeOwned,
    Req: DeserializeOwned,
    Resp: Serialize,
{
    let resp = handler(&params(call.params)?, decode(call.request)?, context)?;
    Ok(to_string(&resp).unwrap_or_default())
}

fn listed<Params, Resp>(
    query: &str,
    context: &Context<'_>,
    handler: Queried<Params, Resp>,
) -> Result<String, AppError>
where
    Params: DeserializeOwned,
    Resp: Serialize,
{
    let resp = handler(&params(query)?, context)?;
    Ok(to_string(&resp).unwrap_or_default())
}

fn params<Params>(query: &str) -> Result<Params, AppError>
where
    Params: DeserializeOwned,
{
    from_query(query).map_err(|_err| AppError::BadRequest)
}

fn route(target: Target<'_>) -> Option<&'static Route> {
    ROUTES
        .iter()
        .find(|hit| hit.feature == target.feature && hit.action == target.action)
}

fn typed<Req, Resp>(request: &str, handler: fn(Req) -> Resp) -> Result<String, AppError>
where
    Req: DeserializeOwned,
    Resp: Serialize,
{
    decode(request).map(|req| to_string(&handler(req)).unwrap_or_default())
}
