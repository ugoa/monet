use crate::{
    Body, BoxError, HttpBody, HttpRequest, HttpResponse, IntoResponse, TowerService,
    routing::{
        route_tower_impl::RouteFuture,
        router::{NotFound, Router},
    },
    serve::{IncomingStream, Listener},
};
use std::{
    convert::Infallible,
    future::ready,
    task::{Context, Poll},
};

impl<'a, L> TowerService<IncomingStream<'_, L>> for Router<'a, ()>
where
    L: Listener,
{
    type Response = Self;

    type Error = Infallible;

    type Future = std::future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: IncomingStream<'_, L>) -> Self::Future {
        std::future::ready(Ok(self.clone().with_state(())))
    }
}

impl<'a, B> TowerService<HttpRequest<'a, B>> for Router<'a, ()>
where
    B: HttpBody<Data = bytes::Bytes> + 'a,
    B::Error: Into<BoxError>,
{
    type Response = HttpResponse<'a>;

    type Error = Infallible;

    type Future = RouteFuture<'a, Infallible>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    // Router is a Tower Service, which is converted to Hyper Service at crate::serve.rs#L129 ,
    // Hyper server will call the hyper service's call() method.
    // Inside hyper's call() method it returns a OneShot future:
    //      https://github.com/hyperium/hyper-util/blob/v0.1.19/src/service/oneshot.rs#L51
    // when the future is being polled by the runtime, the Towerservice call() is triggered,
    // which is below
    fn call(&mut self, req: HttpRequest<'a, B>) -> Self::Future {
        let req = req.map(Body::new);
        self.call_with_state(req, ())
    }
}

impl<B> TowerService<HttpRequest<'_, B>> for NotFound {
    type Response = HttpResponse<'static>;
    type Error = Infallible;
    type Future = std::future::Ready<Result<HttpResponse<'static>, Self::Error>>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: HttpRequest<'_, B>) -> Self::Future {
        ready(Ok(http::StatusCode::NOT_FOUND.into_response()))
    }
}
