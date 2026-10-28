use alloc::sync::Arc;
use core::future::poll_fn;
use core::pin::pin;
use core::task::Poll;
use std::io;

use adapters::ingest::run as ingest_run;
use adapters::race::Stop;
use adapters::triage::{Setup, run as triage_run};
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
    let mut halt = pin!(shutdown());
    let mut collecting = pin!(collect(&settings, &mut stop));
    let mut working = pin!(work(&settings, &mut halt));
    let mut serving = pin!(http::run(listener, Arc::clone(&settings)));
    let mut collected: Option<u8> = None;
    let mut served: Option<Outcome> = None;
    let mut worked: Option<()> = None;
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
        if worked.is_none() && working.as_mut().poll(context).is_ready() {
            worked = Some(());
        }
        match (collected, worked, served.take()) {
            (Some(code), _, _) if code != 0 => Poll::Ready((code, Ok(()))),
            (Some(_), Some(()), Some(done)) => Poll::Ready(done),
            (_, _, held) => {
                served = held;
                Poll::Pending
            }
        }
    })
    .await
}

async fn work(settings: &Settings, stop: Stop<'_>) {
    let shelf = settings.ingest().shelf();
    let context = settings.context(&shelf);
    let setup = Setup::new(settings.ingest(), settings.triage(), &context);
    triage_run(setup, stop)
        .with_subscriber(settings.log().clone())
        .await;
}
