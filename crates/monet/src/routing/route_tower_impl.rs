use crate::prelude::*;
use http::Method;
use pin_project_lite::pin_project;
use std::{
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll, ready},
};
use tower::util::{Oneshot, ServiceExt};

/// A local boxed [`Service`] trait object with `Clone`. Same with UnsyncBoxService
/// Ref: https://github.com/tower-rs/tower/blob/tower-0.5.2/tower/src/util/boxed/unsync.rs#L12

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
pub(crate) struct MapIntoResponse<'a, S> {
    pub inner: S,
    _marker: PhantomData<&'a ()>,
}

impl<S> MapIntoResponse<'_, S> {
    pub(crate) fn new(inner: S) -> Self {
        Self {
            inner,
            _marker: PhantomData,
        }
    }
}

impl<'a, B, S> TowerService<http::Request<B>> for MapIntoResponse<'a, S>
where
    S: TowerService<http::Request<B>> + 'a,
    S::Response: IntoResponse + 'a,
    S::Future: 'a,
{
    type Response = HttpResponse<'a>;
    type Error = S::Error;
    type Future = MapIntoResponseFuture<'a, S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<B>) -> Self::Future {
        MapIntoResponseFuture {
            inner: self.inner.call(req),
            _marker: PhantomData,
        }
    }
}

pin_project! {
    pub(crate) struct MapIntoResponseFuture<'a, F> {
        #[pin]
        pub inner: F,
        _marker: PhantomData<&'a ()>,
    }
}

impl<'a, F, T, E> Future for MapIntoResponseFuture<'a, F>
where
    F: Future<Output = Result<T, E>> + 'a,
    T: IntoResponse,
{
    type Output = Result<HttpResponse<'a>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let res = ready!(self.project().inner.poll(cx)?);

        Poll::Ready(Ok(res.into_response()))
        // Here every different types of return values from handler turn into Response
    }
}

pin_project! {
    /// Response future for [`Route`].
    pub struct RouteFuture<'a, E> {
        #[pin]
        inner: Oneshot<LocalBoxCloneService<'a, HttpRequest<'a>,HttpResponse<'a>,E> , HttpRequest<'a>>,
        method: Method,
    }
}

impl<'a, B, E> TowerService<HttpRequest<'a, B>> for Route<'a, E>
where
    B: HttpBody<Data = bytes::Bytes> + 'a,
    B::Error: Into<BoxError>,
{
    type Response = HttpResponse<'a>;
    type Error = E;
    type Future = RouteFuture<'a, E>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, req: HttpRequest<'a, B>) -> Self::Future {
        self.oneshot_inner(req.map(Body::new))
    }
}

impl<'a, E> RouteFuture<'a, E> {
    pub fn new(
        method: Method,
        inner: Oneshot<
            LocalBoxCloneService<'a, HttpRequest<'a>, HttpResponse<'a>, E>,
            HttpRequest<'a>,
        >,
    ) -> Self {
        Self { inner, method }
    }
}

impl<'a, E> Future for RouteFuture<'a, E> {
    type Output = Result<HttpResponse<'a>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        let resp = std::task::ready!(this.inner.poll(cx))?;

        Poll::Ready(Ok(resp))
    }
}
