use core::future::poll_fn;
use core::task::Poll;

use tokio::signal::unix::{SignalKind, signal};

pub(crate) fn shutdown() -> impl Future<Output = ()> + Send {
    let mut signals =
        [SignalKind::interrupt(), SignalKind::terminate()].map(|kind| signal(kind).ok());
    poll_fn(move |context| {
        if signals
            .iter_mut()
            .flatten()
            .any(|sig| sig.poll_recv(context).is_ready())
        {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    })
}
