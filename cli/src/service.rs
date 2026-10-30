use alloc::sync::Arc;
use core::future::{poll_fn, ready};
use core::pin::{Pin, pin};
use core::task::{Context, Poll};
use std::io;

use adapters::ingest::{Site, run as ingest_run};
use adapters::triage::{Lane, Setup, run as triage_run};
use tokio::net::TcpListener;
use tokio::sync::watch::{Receiver, Sender, channel};
use tokio::task::JoinSet;
use tracing::instrument::WithSubscriber as _;

use crate::config::Settings;
use crate::http;
use crate::signal::shutdown;

struct Crew {
    quit: Sender<bool>,
    signal: Halt,
    tasks: JoinSet<u8>,
    watcher: Receiver<bool>,
}

impl Crew {
    fn halt(&self) -> Halt {
        let mut watcher = self.watcher.clone();
        Box::pin(async move {
            drop(watcher.wait_for(|stop| *stop).await);
        })
    }

    fn launch(&mut self, settings: &Settings, job: Job) {
        drop(
            self.tasks
                .spawn(job.with_subscriber(settings.log().clone())),
        );
    }

    fn new() -> Self {
        let (quit, watcher) = channel(false);
        Self {
            quit,
            signal: Box::pin(shutdown()),
            tasks: JoinSet::new(),
            watcher,
        }
    }

    fn reap(&mut self, context: &mut Context<'_>, failed: u8) -> u8 {
        let mut code: u8 = 0;
        while let Poll::Ready(Some(joined)) = self.tasks.poll_join_next(context) {
            let exit = joined.unwrap_or(failed);
            if exit != 0 {
                self.quit.send_modify(|stop| *stop = true);
            }
            code = code.max(exit);
        }
        code
    }

    fn sources(&self, settings: &Arc<Settings>) -> Vec<Job> {
        (0..settings.ingest().sources().len())
            .map(|index| gather(settings, index, self.halt()))
            .collect()
    }

    async fn supervise(mut self, mut serving: Serving<'_>, failed: u8) -> Outcome {
        let mut code: u8 = 0;
        let mut served: Option<Outcome> = None;
        poll_fn(|context| {
            if self.signal.as_mut().poll(context).is_ready() {
                self.quit.send_modify(|stop| *stop = true);
            }
            if served.is_none()
                && let Poll::Ready(done) = serving.as_mut().poll(context)
            {
                served = Some(done);
            }
            code = code.max(self.reap(context, failed));
            match (self.tasks.is_empty(), code, served.take()) {
                (false, _, held) | (true, 0, held @ None) => {
                    served = held;
                    Poll::Pending
                }
                (true, 0, Some(done)) => Poll::Ready(done),
                (true, failure, _) => Poll::Ready((failure, Ok(()))),
            }
        })
        .await
    }
}

type Halt = Pin<Box<dyn Future<Output = ()> + Send>>;
type Job = Pin<Box<dyn Future<Output = u8> + Send>>;
type Outcome = (u8, io::Result<()>);
type Serving<'serving> = Pin<&'serving mut (dyn Future<Output = Outcome> + Send)>;

pub(crate) async fn collect(settings: Arc<Settings>) -> u8 {
    drop(settings.ingest().prepare());
    let mut crew = Crew::new();
    for job in crew.sources(&settings) {
        crew.launch(&settings, job);
    }
    let failed = settings.ingest().failure_exit_code();
    let idle = pin!(ready((0, Ok(()))));
    crew.supervise(idle, failed).await.0
}

fn gather(settings: &Arc<Settings>, index: usize, halt: Halt) -> Job {
    let owned = Arc::clone(settings);
    Box::pin(async move {
        let mut stop = halt;
        let shelf = owned.ingest().shelf();
        let context = owned.context(&shelf);
        let site = Site::new(owned.ingest(), &context);
        let mut codes = Vec::new();
        for source in owned.ingest().sources().iter().skip(index).take(1) {
            codes.push(ingest_run(site, source, &mut stop).await);
        }
        codes.into_iter().max().unwrap_or_default()
    })
}

pub(crate) async fn run(listener: TcpListener, settings: Arc<Settings>) -> Outcome {
    drop(settings.ingest().prepare());
    let mut crew = Crew::new();
    let mut jobs = crew.sources(&settings);
    jobs.extend([Lane::Batches, Lane::Alerts].map(|lane| work(&settings, lane, crew.halt())));
    let serving = pin!(http::start(listener, Arc::clone(&settings)));
    for job in jobs {
        crew.launch(&settings, job);
    }
    let failed = settings.ingest().failure_exit_code();
    crew.supervise(serving, failed).await
}

fn work(settings: &Arc<Settings>, lane: Lane, halt: Halt) -> Job {
    let owned = Arc::clone(settings);
    Box::pin(async move {
        let mut stop = halt;
        let shelf = owned.ingest().shelf();
        let context = owned.context(&shelf);
        let setup = Setup::new(owned.ingest(), owned.triage(), &context);
        triage_run(setup, lane, &mut stop).await;
        0
    })
}
