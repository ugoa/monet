use std::{
    convert::Infallible,
    marker::PhantomData,
    task::{Context, Poll},
};

use futures::future::Map;

use crate::{
    extract::{FromRequest, FromRequestParts},
    handler::{Handler, HandlerService},
    opaque_future,
    response::IntoResponse,
    Body, BoxError, HttpBody, HttpRequest, HttpResponse, TowerService,
};

impl<'a, H, X, S, B> TowerService<HttpRequest<B>> for HandlerService<'a, H, X, S>
where
    H: Handler<'a, X, S> + Clone + 'a,
    H::Future: 'a,
    B: HttpBody<Data = bytes::Bytes> + 'a + 'static,
    B::Error: Into<BoxError>,
    S: Clone + 'a,
{
    type Response = HttpResponse;

    type Error = Infallible;

    type Future = IntoServiceFuture<<H as Handler<'a, X, S>>::Future>;

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
