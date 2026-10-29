extern crate alloc;

pub mod features;
pub mod inbox;
pub mod review;

use core::num::NonZeroU8;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::to_string;

#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub enum AppError {
    BadRequest,
    Storage,
    UnknownAction,
}

impl AppError {
    #[must_use = "响应体需写回调用方"]
    pub fn body(self, errors: &Errors) -> String {
        let code = &self.spec(errors).code;
        to_string(&Body { code }).unwrap_or_default()
    }

    #[must_use = "退出码需返回给进程"]
    pub const fn exit_code(self, errors: &Errors) -> u8 {
        self.spec(errors).exit_code.get()
    }

    const fn spec(self, errors: &Errors) -> &ErrorSpec {
        match self {
            Self::BadRequest => &errors.bad_request,
            Self::Storage => &errors.storage,
            Self::UnknownAction => &errors.unknown_action,
        }
    }

    #[must_use = "状态码需写回 HTTP 响应"]
    pub const fn status(self) -> u16 {
        match self {
            Self::BadRequest => 400,
            Self::Storage => 503,
            Self::UnknownAction => 404,
        }
    }

    #[must_use = "说明需写入接口说明"]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::BadRequest => "bad request",
            Self::Storage => "storage unavailable",
            Self::UnknownAction => "unknown action",
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
    exit_code: NonZeroU8,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Errors {
    bad_request: ErrorSpec,
    storage: ErrorSpec,
    unknown_action: ErrorSpec,
}
