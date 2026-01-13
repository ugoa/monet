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

impl<'a, H, X, S, B> TowerService<HttpRequest<B>> for HandlerService<'a, H, X, S>
where
    H: Handler<'a, X, S>,
    B: HttpBody<Data = bytes::Bytes> + 'a,
    B::Error: Into<BoxError>,
    S: Clone + 'a,
{
    type Response = HttpResponse;

    type Error = Infallible;

    type Future = IntoServiceFuture<H::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: HttpRequest<B>) -> Self::Future {
        use futures_util::future::FutureExt;
        let req = req.map(Body::new);
        let handler = self.handler.clone();

        let future = Handler::call(handler, req, self.state.clone());

        let future = future.map(Ok as _);

        IntoServiceFuture::new(future)
    }
}

opaque_future! {
    /// The response future for [`IntoService`](super::IntoService).
    pub type IntoServiceFuture<F> =
        Map<
            F,
            fn(HttpResponse) -> Result<HttpResponse, Infallible>,
        >;
}
