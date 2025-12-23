use crate::{HttpRequest, HttpResponse};

use super::route_tower_impl::{MapIntoResponse, RouteFuture};
// use crate::{handler::Handler, prelude::*};
use std::convert::Infallible;
use tower::Service;
use tower::layer::{LayerFn, layer_fn};
// use tower::util::UnsyncBoxService;
use tower::{Layer, ServiceExt, util::MapErrLayer};

use std::fmt;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

// pub struct MonetRoute<E = Infallible>(UnsyncBoxService<HttpRequest, HttpResponse, E>);

/// A boxed [`Service`] trait object.
pub struct UnsyncBoxService<'a, T, U, E> {
    inner: Box<dyn Service<T, Response = U, Error = E, Future = UnsyncBoxFuture<U, E>> + 'a>,
}

/// A boxed [`Future`] trait object.
///
/// This type alias represents a boxed future that is *not* [`Send`] and must
/// remain on the current thread.
type UnsyncBoxFuture<T, E> = Pin<Box<dyn Future<Output = Result<T, E>>>>;

#[derive(Debug)]
struct UnsyncBoxed<S> {
    inner: S,
}

impl<'a, T, U, E> UnsyncBoxService<'a, T, U, E> {
    pub fn new<S>(inner: S) -> Self
    where
        S: Service<T, Response = U, Error = E> + 'a,
        S::Future: 'a,
    {
        let inner = Box::new(inner);
        UnsyncBoxService { inner }
    }

    /// Returns a [`Layer`] for wrapping a [`Service`] in an [`UnsyncBoxService`] middleware.
    ///
    /// [`Layer`]: crate::Layer
    pub fn layer<S>() -> LayerFn<fn(S) -> Self>
    where
        S: Service<T, Response = U, Error = E> + 'static,
        S::Future: 'static,
    {
        layer_fn(Self::new)
    }
}

impl<T, U, E> Service<T> for UnsyncBoxService<T, U, E> {
    type Response = U;
    type Error = E;
    type Future = UnsyncBoxFuture<U, E>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), E>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: T) -> UnsyncBoxFuture<U, E> {
        self.inner.call(request)
    }
}

impl<T, U, E> fmt::Debug for UnsyncBoxService<T, U, E> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("UnsyncBoxService").finish()
    }
}

impl<S, Request> Service<Request> for UnsyncBoxed<S>
where
    S: Service<Request> + 'static,
    S::Future: 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<S::Response, S::Error>>>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        Box::pin(self.inner.call(request))
    }
}
