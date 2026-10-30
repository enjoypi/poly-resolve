use core::future::{IntoFuture as _, poll_fn};
use core::net::SocketAddr;
use core::pin::pin;
use core::task::Poll;
use core::time::Duration;
use std::io;

use alloc::sync::Arc;

use axum::Router;
use axum::extract::{RawPathParams, RawQuery, Request, State};
use axum::http::header::{CONTENT_TYPE, HOST};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::middleware::{Next, from_fn_with_state};
use axum::response::{IntoResponse as _, Response};
use axum::routing::{MethodFilter, MethodRouter, get, on};
use axum::serve;
use domain::features::catalog::{Entry, build};
use domain::features::{self, Call, Origin, Target};
use serde_urlencoded::to_string as encode;
use tokio::net::TcpListener;
use tokio::time::sleep;
use tracing::dispatcher::with_default;

use crate::assets;
use crate::config::Settings;
use crate::signal::shutdown;

const JSON: &str = "application/json";
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

fn endpoint(entry: &Entry) -> MethodRouter<Arc<Settings>> {
    let (feature, action) = (entry.feature(), entry.action());
    let filter = Method::from_bytes(entry.method().to_uppercase().as_bytes()).unwrap_or_default();
    on(
        MethodFilter::try_from(filter).unwrap_or(MethodFilter::GET),
        async move |State(settings): State<Arc<Settings>>,
                    path: RawPathParams,
                    RawQuery(query): RawQuery,
                    headers: HeaderMap,
                    body: String| {
            let params = params(&path, query);
            let origin = Origin::new(
                "http",
                header(&headers, SESSION_HEADER),
                header(&headers, USER_HEADER),
            );
            let call = Call::new(origin, Target::new(feature, action), &body).with_params(&params);
            respond(&settings, &call)
        },
    )
}

async fn guard(State(hosts): State<Arc<[String]>>, request: Request, next: Next) -> Response {
    let headers = request.headers();
    let known = header(headers, HOST.as_str())
        .is_some_and(|host| hosts.iter().any(|allowed| allowed == host));
    let typed = request.method() == Method::GET
        || header(headers, CONTENT_TYPE.as_str()).is_some_and(|kind| kind.starts_with(JSON));
    if !known {
        StatusCode::FORBIDDEN.into_response()
    } else if typed {
        next.run(request).await
    } else {
        StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response()
    }
}

fn header<'headers>(headers: &'headers HeaderMap, name: &str) -> Option<&'headers str> {
    headers.get(name)?.to_str().ok()
}

fn hosts(bound: Option<SocketAddr>) -> Vec<String> {
    bound.map_or_default(|addr| vec![addr.to_string(), format!("localhost:{}", addr.port())])
}

fn params(path: &RawPathParams, query: Option<String>) -> String {
    let pairs: Vec<(&str, &str)> = path.iter().collect();
    [query.unwrap_or_default(), encode(pairs).unwrap_or_default()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<String>>()
        .join("&")
}

fn respond(settings: &Settings, call: &Call<'_>) -> Response {
    let shelf = settings.ingest().shelf();
    let context = settings.context(&shelf);
    let result = with_default(settings.log(), || features::dispatch(&context, call));
    let (status, response) = match result {
        Ok(response) => (StatusCode::OK, response),
        Err(err) => (
            StatusCode::from_u16(err.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            err.body(settings.errors()),
        ),
    };
    (status, [(CONTENT_TYPE, JSON)], response).into_response()
}

pub(crate) async fn run(listener: TcpListener, settings: Arc<Settings>) -> (u8, io::Result<()>) {
    let (graceful, expiry) = (shutdown(), shutdown());
    let bound = listener.local_addr().ok();
    let addr = bound.map_or_default(|local| local.to_string());
    with_default(settings.log(), || tracing::info!(addr, "listening"));
    let pages = Router::new()
        .route("/", get(assets::index))
        .route("/{file}", get(assets::file));
    let app = build()
        .entries()
        .iter()
        .fold(pages, |router, entry| {
            router.route(entry.path(), endpoint(entry))
        })
        .layer(from_fn_with_state(Arc::from(hosts(bound)), guard))
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
