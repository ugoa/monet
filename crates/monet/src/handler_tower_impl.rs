use std::{
    convert::Infallible,
    marker::PhantomData,
    task::{Context, Poll},
};

use futures::future::Map;

use crate::{
    Body, BoxError, HttpBody, HttpRequest, HttpResponse, TowerService,
    extract::{FromRequest, FromRequestParts},
    handler::{Handler, HandlerService},
    opaque_future,
    response::IntoResponse,
};

impl<'a, H, X, S, B> TowerService<HttpRequest<'a, B>> for HandlerService<'a, H, X, S>
where
    H: Handler<'a, X, S> + Clone + 'a,
    B: HttpBody<Data = bytes::Bytes> + 'a,
    B::Error: Into<BoxError>,
    S: Clone + 'a,
{
    type Response = HttpResponse<'a>;

    type Error = Infallible;

    type Future = IntoServiceFuture<'a, H::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: HttpRequest<'a, B>) -> Self::Future {
        use futures_util::future::FutureExt;
        let req = req.map(Body::new);
        let handler = self.handler.clone();

        let future = Handler::call(handler, req, self.state.clone());

        IntoServiceFuture::new(future)
    }
}

pin_project_lite::pin_project! {
    /// The response future for [`IntoService`](super::IntoService).
    pub struct IntoServiceFuture<'a, F> {
        #[pin] future: futures::future::Map<F, fn(HttpResponse<'a>) -> Result<HttpResponse<'a>, Infallible>>,
    }
}

impl<'a, F> IntoServiceFuture<'a, F> {
    pub(crate) fn new(future: F) -> Self
    where
        F: std::future::Future<Output = HttpResponse<'a>>,
    {
        use futures::future::FutureExt;
        Self {
            future: future.map(Ok as _),
        }
    }
}

impl<'a, F> std::fmt::Debug for IntoServiceFuture<'a, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IntoServiceFuture").finish_non_exhaustive()
    }
}

impl<'a, F> std::future::Future for IntoServiceFuture<'a, F>
where
    F: std::future::Future<Output = HttpResponse<'a>>,
{
    type Output = Result<HttpResponse<'a>, Infallible>;

    #[inline]
    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.project();
        this.future.poll(cx)
    }
}
