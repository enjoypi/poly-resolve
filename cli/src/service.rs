use alloc::sync::Arc;
use core::future::poll_fn;
use core::pin::pin;
use core::task::Poll;
use std::io;

use adapters::ingest::run as ingest_run;
use adapters::race::Stop;
use tokio::net::TcpListener;
use tracing::instrument::WithSubscriber as _;

use crate::config::Settings;
use crate::http;
use crate::signal::shutdown;

type Outcome = (u8, io::Result<()>);

pub(crate) async fn collect(settings: &Settings, stop: Stop<'_>) -> u8 {
    let shelf = settings.ingest().shelf();
    let context = settings.context(&shelf);
    ingest_run(settings.ingest(), &context, stop)
        .with_subscriber(settings.log().clone())
        .await
}

pub(crate) async fn run(listener: TcpListener, settings: Arc<Settings>) -> Outcome {
    let mut stop = pin!(shutdown());
    let mut collecting = pin!(collect(&settings, &mut stop));
    let mut serving = pin!(http::run(listener, Arc::clone(&settings)));
    let mut collected: Option<u8> = None;
    let mut served: Option<Outcome> = None;
    poll_fn(|context| {
        if served.is_none()
            && let Poll::Ready(done) = serving.as_mut().poll(context)
        {
            served = Some(done);
        }
        if collected.is_none()
            && let Poll::Ready(code) = collecting.as_mut().poll(context)
        {
            collected = Some(code);
        }
        match (collected, served.take()) {
            (Some(code), _) if code != 0 => Poll::Ready((code, Ok(()))),
            (Some(_), Some(done)) => Poll::Ready(done),
            (_, held) => {
                served = held;
                Poll::Pending
            }
        }
    })
    .await
}
