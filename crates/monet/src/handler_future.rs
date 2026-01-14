use std::{
    convert::Infallible,
    fmt,
    pin::Pin,
    task::{Context, Poll},
};

use crate::{HttpResponse, opaque_future};
use futures::future::Map;
use pin_project_lite::pin_project;
use std::future::Future;

pin_project! {
    pub struct IntoServiceFuture<F> {
        #[pin]
        future: Map<F, fn(HttpResponse) -> Result<HttpResponse, Infallible>>,
    }
}

impl<F> IntoServiceFuture<F> {
    pub(crate) fn new(
        future: Map<F, fn(HttpResponse) -> Result<HttpResponse, Infallible>>,
    ) -> Self {
        Self { future }
    }
}

impl<F> fmt::Debug for IntoServiceFuture<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IntoServiceFuture").finish_non_exhaustive()
    }
}

impl<F> Future for IntoServiceFuture<F>
where
    Map<F, fn(HttpResponse) -> Result<HttpResponse, Infallible>>: Future,
{
    type Output = <Map<F, fn(HttpResponse) -> Result<HttpResponse, Infallible>> as Future>::Output;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().future.poll(cx)
    }
}
