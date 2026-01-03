use bytes::Bytes;
use http::Method;
use pin_project_lite::pin_project;
use std::{
    convert::Infallible,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll, ready},
};
use tower::util::{Oneshot, ServiceExt};

use tower::Service as TowerService;

use crate::{BoxError, HttpBody, HttpResponse, IntoResponse, opaque_future};

pub struct Body<'a>(Pin<Box<dyn HttpBody<Data = Bytes, Error = BoxError> + 'a>>);
pub type HttpRequest<'a, T = Body<'a>> = http::Request<T>;

pub struct Route<'r, E = Infallible>(LocalBoxCloneService<'r, HttpRequest<'r>, HttpResponse, E>);

impl<'svc, 'resp, 'fut, E> Route<'svc, E> {
    pub fn new<T>(svc: T) -> Self
    where
        T: TowerService<HttpRequest<'svc>, Error = E> + Clone + 'svc,
        T::Response: IntoResponse + 'resp,
        T::Future: 'fut,
        'svc: 'fut,
        'resp: 'fut,
    {
        Self(LocalBoxCloneService::new(MapIntoResponse::new(svc)))
    }
}

pub(crate) struct BoxedIntoRoute<'a, S, E>(pub Box<dyn ErasedIntoRoute<'a, S, E> + 'a>);

pub(crate) trait ErasedIntoRoute<'a, S, E> {
    fn clone_box(&self) -> Box<dyn ErasedIntoRoute<'a, S, E> + 'a>;

    fn into_route(self: Box<Self>, state: S) -> Route<'a, E>;
}

pub struct ErasedHandler<'a, H, S> {
    pub handler: H,
    pub into_route_fn: fn(H, S) -> Route<'a>,
}

impl<'a, H, S> ErasedIntoRoute<'a, S, Infallible> for ErasedHandler<'a, H, S>
where
    H: Clone + 'a,
    S: 'a,
{
    fn clone_box(&self) -> Box<dyn ErasedIntoRoute<'a, S, Infallible> + 'a> {
        Box::new(self.clone())
    }

    fn into_route(self: Box<Self>, state: S) -> Route<'a, Infallible> {
        (self.into_route_fn)(self.handler, state)
    }
}

impl<'a, H, S> Clone for ErasedHandler<'a, H, S>
where
    H: Clone + 'a,
{
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            into_route_fn: self.into_route_fn,
        }
    }
}

// ///  Transfer handler to Route
// impl<'a, S> BoxedIntoRoute<'a, S, Infallible>
// where
//     S: Clone + 'a,
// {
//     pub fn from_handler<H, X>(handler: H) -> Self
//     where
//         H: Handler<'a, X, S> + 'a,
//         X: 'a,
//     {
//         let svc_fn = |handler, state| {
//             let svc = HandlerService::new(handler, state);
//             let resp_map = MapIntoResponse::new(svc);
//             let lbcs = LocalBoxCloneService::new(resp_map);
//             Route(lbcs)
//         };
//         let erased = ErasedHandler {
//             handler: handler,
//             into_route_fn: svc_fn,
//         };
//         BoxedIntoRoute(Box::new(erased))
//     }
// }

pub trait Handler<'a, X, S>: Clone + Sized + 'a
where
    X: 'a,
    S: 'a,
{
    type Future: Future<Output = HttpResponse> + 'a;

    fn call(self, req: HttpRequest, state: S) -> Self::Future;

    fn to_service(self, state: S) -> HandlerService<Self, X, S> {
        HandlerService::new(self, state)
    }
}

pub struct HandlerService<H, X, S> {
    pub handler: H,
    pub state: S,
    pub(crate) _marker: PhantomData<fn() -> X>,
}

impl<H, X, S> Clone for HandlerService<H, X, S>
where
    H: Clone,
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            state: self.state.clone(),
            _marker: PhantomData,
        }
    }
}

impl<H, X, S> HandlerService<H, X, S> {
    pub(super) fn new(handler: H, state: S) -> Self {
        Self {
            handler,
            state,
            _marker: PhantomData,
        }
    }
    pub fn state(&self) -> &S {
        &self.state
    }
}

// impl<'a, H, X, S, B> TowerService<HttpRequest<'a, B>> for HandlerService<H, X, S>
// where
//     H: Handler<'a, X, S> + Clone + 'a,
//     B: HttpBody<Data = bytes::Bytes>,
//     B::Error: Into<BoxError>,
//     S: Clone + 'a,
// {
//     type Response = HttpResponse;
//
//     type Error = Infallible;
//
//     type Future = IntoServiceFuture<H::Future>;
//
//     fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
//         Poll::Ready(Ok(()))
//     }
//
//     fn call(&mut self, req: HttpRequest<B>) -> Self::Future {
//         use futures_util::future::FutureExt;
//         let req = req.map(Body::new);
//         let handler = self.handler.clone();
//
//         let future = Handler::call(handler, req, self.state.clone());
//
//         let future = future.map(Ok as _);
//
//         IntoServiceFuture::new(future)
//     }
// }
//
//
// use http_body_util::BodyExt;
// impl<'a> Body<'a> {
//     pub fn new<B>(http_body: B) -> Self
//     where
//         B: HttpBody<Data = Bytes> + 'a,
//         B::Error: Into<BoxError>,
//     {
//         let body = http_body.map_err(Into::into);
//         Body(Box::pin(body))
//     }
//
//     pub fn empty() -> Self {
//         Self::new(http_body_util::Empty::new())
//     }
// }
//
// use futures::future::Map;
// opaque_future! {
//     /// The response future for [`IntoService`](super::IntoService).
//     pub type IntoServiceFuture<F> =
//         Map<
//             F,
//             fn(HttpResponse) -> Result<HttpResponse, Infallible>,
//         >;
// }

pub struct LocalBoxCloneService<'a, T, U, E>(
    Box<
        dyn ClonableService<
                'a,
                T,
                Response = U,
                Error = E,
                Future = Pin<Box<dyn Future<Output = Result<U, E>> + 'a>>,
            > + 'a,
    >,
);

impl<'a, T, U, E> LocalBoxCloneService<'a, T, U, E> {
    /// Create a new `BoxCloneSyncService`.
    pub fn new<S>(inner: S) -> Self
    where
        S: TowerService<T, Response = U, Error = E> + Clone + 'a,
        <S as tower::Service<T>>::Future: 'a,
    {
        let inner = inner.map_future(|fut| Box::pin(fut) as _);
        LocalBoxCloneService(Box::new(inner))
    }
}

impl<'a, T, U, E> TowerService<T> for LocalBoxCloneService<'a, T, U, E> {
    type Response = U;

    type Error = E;

    type Future = Pin<Box<dyn Future<Output = Result<U, E>> + 'a>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready(cx)
    }

    fn call(&mut self, req: T) -> Self::Future {
        self.0.call(req)
    }
}

impl<T, U, E> Clone for LocalBoxCloneService<'_, T, U, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

trait ClonableService<'a, S>: TowerService<S> {
    fn clone_box(
        &self,
    ) -> Box<
        dyn ClonableService<
                'a,
                S,
                Response = Self::Response,
                Error = Self::Error,
                Future = Self::Future,
            > + 'a,
    >;
}

impl<'a, S, T> ClonableService<'a, S> for T
where
    T: TowerService<S> + Clone + 'a,
{
    fn clone_box(
        &self,
    ) -> Box<
        dyn ClonableService<'a, S, Response = T::Response, Error = T::Error, Future = T::Future>
            + 'a,
    > {
        Box::new(self.clone())
    }
}

#[derive(Clone)]
pub(crate) struct MapIntoResponse<S> {
    pub inner: S,
}

impl<S> MapIntoResponse<S> {
    pub(crate) fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<'a, B, S> TowerService<http::Request<B>> for MapIntoResponse<S>
where
    S: TowerService<http::Request<B>> + 'a,
    S::Response: IntoResponse + 'a,
    S::Future: 'a,
{
    type Response = HttpResponse;
    type Error = S::Error;
    type Future = MapIntoResponseFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<B>) -> Self::Future {
        MapIntoResponseFuture {
            inner: self.inner.call(req),
        }
    }
}

pin_project! {
    pub(crate) struct MapIntoResponseFuture<F> {
        #[pin]
        pub inner: F,
    }
}

impl<F, T, E> Future for MapIntoResponseFuture<F>
where
    F: Future<Output = Result<T, E>>,
    T: IntoResponse,
{
    type Output = Result<HttpResponse, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let res = ready!(self.project().inner.poll(cx)?);

        Poll::Ready(Ok(res.into_response()))
        // Here every different types of return values from handler turn into Response
    }
}
