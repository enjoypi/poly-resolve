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
use serde_json::{Map, Value, from_str, to_string};

use crate::inbox::{Paging, Reader};
use crate::review::{Docket, Reason};
use crate::{AppError, Errors};

static ROUTES: [Route; 20] = [
    Route {
        action: "check",
        call: |call, _context| typed(call.request, health::check),
        failures: &[],
        feature: "health",
        schema: schemas::<Map<String, Value>, health::Response>,
        trace: Trace::Full,
    },
    Route {
        action: "assemble",
        call: |call, _context| typed(call.request, ingest::assemble),
        failures: &[],
        feature: "ingest",
        schema: schemas::<ingest::AssembleRequest, ingest::Envelope>,
        trace: Trace::Redacted,
    },
    Route {
        action: "cipher",
        call: |call, _context| typed(call.request, ingest::cipher),
        failures: &[],
        feature: "ingest",
        schema: schemas::<ingest::CipherRequest, ingest::CipherResponse>,
        trace: Trace::Redacted,
    },
    Route {
        action: "parse_event",
        call: |call, _context| fallible(call.request, ingest::parse_event),
        failures: &[],
        feature: "ingest",
        schema: schemas::<ingest::ParseRequest, ingest::Envelope>,
        trace: Trace::Redacted,
    },
    Route {
        action: "parse_page",
        call: |call, _context| fallible(call.request, ingest::parse_page),
        failures: &[],
        feature: "ingest",
        schema: schemas::<ingest::ParseRequest, ingest::PageResponse>,
        trace: Trace::Redacted,
    },
    Route {
        action: "reveal",
        call: |call, _context| typed(call.request, ingest::reveal),
        failures: &[],
        feature: "ingest",
        schema: schemas::<ingest::RevealRequest, ingest::Envelope>,
        trace: Trace::Redacted,
    },
    Route {
        action: "window",
        call: |call, _context| fallible(call.request, ingest::window),
        failures: &[],
        feature: "ingest",
        schema: schemas::<ingest::WindowRequest, ingest::WindowResponse>,
        trace: Trace::Full,
    },
    Route {
        action: "list",
        call: |call, context| queried(call.request, context, messages::list),
        failures: &[AppError::Storage],
        feature: "messages",
        schema: schemas::<messages::ListRequest, messages::ListResponse>,
        trace: Trace::Redacted,
    },
    Route {
        action: "approve",
        call: |call, context| signed(call, context, review::approve),
        failures: &[AppError::Storage],
        feature: "review",
        schema: schemas::<review::ApproveRequest, review::ApproveResponse>,
        trace: Trace::Redacted,
    },
    Route {
        action: "list",
        call: |call, context| queried(call.request, context, review::list),
        failures: &[AppError::Storage],
        feature: "review",
        schema: schemas::<review::ListRequest, review::ListResponse>,
        trace: Trace::Redacted,
    },
    Route {
        action: "reject",
        call: |call, context| signed(call, context, review::reject),
        failures: &[AppError::Storage],
        feature: "review",
        schema: schemas::<review::RejectRequest, review::RulingResponse>,
        trace: Trace::Redacted,
    },
    Route {
        action: "brief",
        call: |call, _context| fallible(call.request, triage::prompt::brief),
        failures: &[],
        feature: "triage",
        schema: schemas::<triage::prompt::BriefRequest, triage::prompt::BriefResponse>,
        trace: Trace::Redacted,
    },
    Route {
        action: "line",
        call: |call, _context| fallible(call.request, triage::line::line),
        failures: &[],
        feature: "triage",
        schema: schemas::<triage::line::LineRequest, triage::line::LineResponse>,
        trace: Trace::Redacted,
    },
    Route {
        action: "notice",
        call: |call, _context| typed(call.request, triage::prompt::notice),
        failures: &[],
        feature: "triage",
        schema: schemas::<triage::prompt::NoticeRequest, triage::prompt::NoticeResponse>,
        trace: Trace::Redacted,
    },
    Route {
        action: "pack",
        call: |call, _context| fallible(call.request, triage::pack::pack),
        failures: &[],
        feature: "triage",
        schema: schemas::<triage::pack::PackRequest, triage::pack::PackResponse>,
        trace: Trace::Redacted,
    },
    Route {
        action: "redact",
        call: |call, _context| fallible(call.request, triage::redact::redact),
        failures: &[],
        feature: "triage",
        schema: schemas::<triage::redact::RedactRequest, triage::redact::RedactResponse>,
        trace: Trace::Redacted,
    },
    Route {
        action: "reply",
        call: |call, _context| fallible(call.request, triage::reply::reply),
        failures: &[],
        feature: "triage",
        schema: schemas::<triage::reply::ReplyRequest, triage::reply::ReplyResponse>,
        trace: Trace::Redacted,
    },
    Route {
        action: "screen",
        call: |call, _context| typed(call.request, triage::screen::screen),
        failures: &[],
        feature: "triage",
        schema: schemas::<triage::screen::ScreenRequest, triage::screen::ScreenResponse>,
        trace: Trace::Redacted,
    },
    Route {
        action: "screened",
        call: |call, _context| fallible(call.request, triage::screened::screened),
        failures: &[],
        feature: "triage",
        schema: schemas::<triage::screened::ScreenedRequest, triage::screened::ScreenedResponse>,
        trace: Trace::Redacted,
    },
    Route {
        action: "sort",
        call: |call, _context| fallible(call.request, triage::sort::sort),
        failures: &[],
        feature: "triage",
        schema: schemas::<triage::sort::SortRequest, triage::sort::SortResponse>,
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

type Describer = fn(&mut SchemaGenerator) -> (Schema, Schema);

type Fallible<Req, Resp> = fn(&Req) -> Result<Resp, AppError>;

type Handler = fn(&Call<'_>, &Context<'_>) -> Result<String, AppError>;

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
    schema: Describer,
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

type Signed<Req, Resp> = fn(&Req, &Context<'_>, &str) -> Result<Resp, AppError>;

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

fn signed<Req, Resp>(
    call: &Call<'_>,
    context: &Context<'_>,
    handler: Signed<Req, Resp>,
) -> Result<String, AppError>
where
    Req: DeserializeOwned,
    Resp: Serialize,
{
    let user = call
        .origin
        .user
        .filter(|name| !name.is_empty())
        .ok_or(AppError::BadRequest)?;
    let resp = handler(&decode(call.request)?, context, user)?;
    Ok(to_string(&resp).unwrap_or_default())
}

fn typed<Req, Resp>(request: &str, handler: fn(Req) -> Resp) -> Result<String, AppError>
where
    Req: DeserializeOwned,
    Resp: Serialize,
{
    decode(request).map(|req| to_string(&handler(req)).unwrap_or_default())
}
