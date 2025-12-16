use std::{
    convert::Infallible,
    fmt,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use futures::future::Map;

use crate::{
    Body, BoxError, Bytes, HttpBody, HttpRequest, Request, Response, TowerService,
    extract::{FromRequest, FromRequestParts},
    handler::{Handler, HandlerService, IntoServiceFuture},
    opaque_future,
    response::IntoResponse,
};

impl<H, X, S, B> TowerService<Request<B>> for HandlerService<H, X, S>
where
    H: Handler<X, S> + Clone + 'static,
    B: HttpBody<Data = bytes::Bytes> + 'static,
    B::Error: Into<BoxError>,
    S: Clone,
{
    type Response = Response;

    type Error = Infallible;

    type Future = IntoServiceFuture<H::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        use futures_util::future::FutureExt;
        let req = req.map(Body::new);
        let handler = self.handler.clone();

        let future = Handler::call(handler, req, self.state.clone());

        let future = future.map(Ok as _);

        IntoServiceFuture::new(future)
    }
}
