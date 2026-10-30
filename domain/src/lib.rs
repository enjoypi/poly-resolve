extern crate alloc;

pub mod features;
pub mod inbox;
pub mod review;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::to_string;

#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub enum AppError {
    BadRequest,
    Conflict,
    NotFound,
    Storage,
    UnknownAction,
    Unprocessable,
}

impl AppError {
    #[must_use = "响应体需写回调用方"]
    pub fn body(self, errors: &Errors) -> String {
        let code = &self.spec(errors).code;
        to_string(&Body { code }).unwrap_or_default()
    }

    const fn spec(self, errors: &Errors) -> &ErrorSpec {
        match self {
            Self::BadRequest => &errors.bad_request,
            Self::Conflict => &errors.conflict,
            Self::NotFound => &errors.not_found,
            Self::Storage => &errors.storage,
            Self::UnknownAction => &errors.unknown_action,
            Self::Unprocessable => &errors.unprocessable,
        }
    }

    #[must_use = "状态码需写回 HTTP 响应"]
    pub const fn status(self) -> u16 {
        match self {
            Self::BadRequest => 400,
            Self::Conflict => 409,
            Self::NotFound | Self::UnknownAction => 404,
            Self::Storage => 503,
            Self::Unprocessable => 422,
        }
    }

    #[must_use = "说明需写入接口说明"]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::BadRequest => "bad request",
            Self::Conflict => "already settled differently",
            Self::NotFound => "not found",
            Self::Storage => "storage unavailable",
            Self::UnknownAction => "unknown action",
            Self::Unprocessable => "violates review rules",
        }
    }
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "ErrorBody")]
struct Body<'code> {
    code: &'code str,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ErrorSpec {
    code: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Errors {
    bad_request: ErrorSpec,
    conflict: ErrorSpec,
    not_found: ErrorSpec,
    storage: ErrorSpec,
    unknown_action: ErrorSpec,
    unprocessable: ErrorSpec,
}
