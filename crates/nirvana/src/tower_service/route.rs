pub mod box_clone_service;

pub mod make_into_response;

use crate::{
    Body, BoxError, HttpBody, Request, Response, Router, TowerService,
    routing::RouteFuture,
    serve::{IncomingStream, Listener},
};
use std::{
    convert::Infallible,
    pin::Pin,
    task::{Context, Poll},
};

use http::Method;
use pin_project_lite::pin_project;
use tower::util::Oneshot;

use crate::routing::Route;

impl<B, E> TowerService<Request<B>> for Route<E>
where
    B: HttpBody<Data = bytes::Bytes> + 'static,
    B::Error: Into<BoxError>,
{
    type Response = Response;

    type Error = E;

    type Future = RouteFuture<E>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        self.oneshot_inner(req.map(Body::new))
    }
}
