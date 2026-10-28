use core::future::{IntoFuture as _, poll_fn};
use core::pin::pin;
use core::task::Poll;
use core::time::Duration;
use std::io;

use alloc::sync::Arc;

use axum::Router;
use axum::extract::{Path, State};
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse as _, Response};
use axum::routing::post;
use axum::serve;
use domain::features::{self, Call, Origin, Target};
use tokio::net::TcpListener;
use tokio::time::sleep;
use tracing::dispatcher::with_default;

use crate::config::Settings;
use crate::signal::shutdown;

pub(crate) const SESSION_HEADER: &str = "x-session-id";
pub(crate) const USER_HEADER: &str = "x-user";

async fn deadline(signal: impl Future<Output = ()>, settings: &Settings) {
    signal.await;
    with_default(settings.log(), || {
        tracing::info!(
            timeout_ms = settings.shutdown_timeout_ms(),
            "shutdown signal received"
        );
    });
    sleep(Duration::from_millis(settings.shutdown_timeout_ms())).await;
}

async fn handle(
    State(settings): State<Arc<Settings>>,
    Path((feature, action)): Path<(String, String)>,
    (headers, body): (HeaderMap, String),
) -> Response {
    let origin = Origin::new(
        "http",
        header(&headers, SESSION_HEADER),
        header(&headers, USER_HEADER),
    );
    let call = Call::new(origin, Target::new(&feature, &action), &body);
    let result = with_default(settings.log(), || {
        features::dispatch(settings.errors(), &call)
    });
    let (status, response) = match result {
        Ok(response) => (StatusCode::OK, response),
        Err(err) => (
            StatusCode::from_u16(err.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            err.body(settings.errors()),
        ),
    };
    (status, [(CONTENT_TYPE, "application/json")], response).into_response()
}

fn header<'headers>(headers: &'headers HeaderMap, name: &str) -> Option<&'headers str> {
    headers.get(name)?.to_str().ok()
}

pub(crate) async fn run(listener: TcpListener, settings: Arc<Settings>) -> (u8, io::Result<()>) {
    let (graceful, expiry) = (shutdown(), shutdown());
    let addr = listener
        .local_addr()
        .map_or_default(|addr| addr.to_string());
    with_default(settings.log(), || tracing::info!(addr, "listening"));
    let app = Router::new()
        .route("/{feature}/{action}", post(handle))
        .with_state(Arc::clone(&settings));
    let mut server = pin!(
        serve(listener, app)
            .with_graceful_shutdown(graceful)
            .into_future()
    );
    let mut limit = pin!(deadline(expiry, &settings));
    poll_fn(|context| {
        let expired = limit.as_mut().poll(context).is_ready();
        if let Poll::Ready(result) = server.as_mut().poll(context) {
            with_default(settings.log(), || tracing::info!("shutdown complete"));
            Poll::Ready((0, result))
        } else if expired {
            with_default(settings.log(), || {
                tracing::error!(
                    timeout_ms = settings.shutdown_timeout_ms(),
                    exit_code = settings.shutdown_timeout_exit_code(),
                    "shutdown timeout"
                );
            });
            Poll::Ready((settings.shutdown_timeout_exit_code(), Ok(())))
        } else {
            Poll::Pending
        }
    })
    .await
}
