use core::future::poll_fn;
use core::pin::Pin;
use core::task::Poll;

pub type Stop<'stop> = &'stop mut (dyn Future<Output = ()> + Send + Unpin);

pub(crate) async fn race<T>(
    stop: Stop<'_>,
    mut work: Pin<&mut (dyn Future<Output = T> + Send)>,
) -> Option<T> {
    poll_fn(|context| {
        if Pin::new(&mut *stop).poll(context).is_ready() {
            Poll::Ready(None)
        } else {
            work.as_mut().poll(context).map(Some)
        }
    })
    .await
}
