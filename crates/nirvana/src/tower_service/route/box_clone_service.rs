use crate::{
    Body, BoxError, HttpBody, Request, Response, Router, TowerService,
    serve::{IncomingStream, Listener},
};
use std::{
    convert::Infallible,
    pin::Pin,
    task::{Context, Poll},
};
use tower::util::ServiceExt;

/// A local boxed [`Service`] trait object with `Clone`. Same with UnsyncBoxService
/// Ref: https://github.com/tower-rs/tower/blob/tower-0.5.2/tower/src/util/boxed/unsync.rs#L12
pub struct LocalBoxCloneService<T, U, E>(
    Box<
        dyn ClonableService<
                T,
                Response = U,
                Error = E,
                Future = Pin<Box<dyn Future<Output = Result<U, E>>>>,
            >,
    >,
);

impl<T, U, E> LocalBoxCloneService<T, U, E> {
    /// Create a new `BoxCloneSyncService`.
    pub fn new<S>(inner: S) -> Self
    where
        S: TowerService<T, Response = U, Error = E> + Clone + 'static,
        S::Future: 'static,
    {
        let inner = inner.map_future(|f| Box::pin(f) as _);
        LocalBoxCloneService(Box::new(inner))
    }
}

impl<T, U, E> Clone for LocalBoxCloneService<T, U, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

impl<T, U, E> TowerService<T> for LocalBoxCloneService<T, U, E> {
    type Response = U;

    type Error = E;

    type Future = Pin<Box<dyn Future<Output = Result<U, E>>>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready(cx)
    }

    fn call(&mut self, req: T) -> Self::Future {
        self.0.call(req)
    }
}

trait ClonableService<S>: TowerService<S> {
    fn clone_box(
        &self,
    ) -> Box<
        dyn ClonableService<
                S,
                Response = Self::Response,
                Error = Self::Error,
                Future = Self::Future,
            >,
    >;
}

impl<S, T> ClonableService<S> for T
where
    T: TowerService<S> + Clone + 'static,
{
    fn clone_box(
        &self,
    ) -> Box<dyn ClonableService<S, Response = T::Response, Error = T::Error, Future = T::Future>>
    {
        Box::new(self.clone())
    }
}
