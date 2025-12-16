use crate::{
    Body, BoxError, HttpBody, HttpRequest, Request, Response, Router, TowerService,
    response::IntoResponse,
    routing::MapIntoResponseFuture,
    serve::{IncomingStream, Listener},
};
use std::{
    convert::Infallible,
    pin::Pin,
    task::{Context, Poll},
};

#[derive(Clone)]
pub(crate) struct MapIntoResponse<S> {
    pub inner: S,
}

impl<S> MapIntoResponse<S> {
    pub(crate) fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<B, S> TowerService<HttpRequest<B>> for MapIntoResponse<S>
where
    S: TowerService<HttpRequest<B>>,
    S::Response: IntoResponse,
{
    type Response = Response;
    type Error = S::Error;
    type Future = MapIntoResponseFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: HttpRequest<B>) -> Self::Future {
        MapIntoResponseFuture {
            inner: self.inner.call(req),
        }
    }
}
