use alloc::sync::Arc;
use std::ffi::OsStr;
use std::path::Path as FilePath;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse as _, Response};
use tokio::fs::read;

use crate::config::Settings;

const FALLBACK: &str = "application/octet-stream";
const INDEX: &str = "index.html";
const TYPES: [(&str, &str); 4] = [
    ("css", "text/css; charset=utf-8"),
    ("html", "text/html; charset=utf-8"),
    ("js", "text/javascript; charset=utf-8"),
    ("svg", "image/svg+xml"),
];

pub(crate) async fn file(
    State(settings): State<Arc<Settings>>,
    Path(name): Path<String>,
) -> Response {
    serve(settings.web_dir(), &name).await
}

pub(crate) async fn index(State(settings): State<Arc<Settings>>) -> Response {
    serve(settings.web_dir(), INDEX).await
}

fn kind(name: &str) -> &'static str {
    FilePath::new(name)
        .extension()
        .and_then(OsStr::to_str)
        .and_then(|ext| TYPES.iter().find(|entry| entry.0 == ext))
        .map_or(FALLBACK, |entry| entry.1)
}

async fn serve(dir: &FilePath, name: &str) -> Response {
    if !valid(name) {
        return StatusCode::NOT_FOUND.into_response();
    }
    match read(dir.join(name)).await {
        Ok(body) => ([(CONTENT_TYPE, kind(name))], body).into_response(),
        Err(_err) => StatusCode::NOT_FOUND.into_response(),
    }
}

fn valid(name: &str) -> bool {
    !name.starts_with('.')
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}
