use crate::prelude::*;
use http::Method;
use pin_project_lite::pin_project;
use std::{
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
