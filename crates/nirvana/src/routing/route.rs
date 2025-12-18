use super::route_tower::{LocalBoxCloneService, MapIntoResponse, RouteFuture};
use crate::prelude::*;
use std::convert::Infallible;
use tower::{Layer, ServiceExt, util::MapErrLayer};

pub struct Route<E = Infallible>(LocalBoxCloneService<Request, Response, E>);

impl<E> Route<E> {
    pub fn new<T>(svc: T) -> Self
    where
        T: TowerService<Request, Error = E> + Clone + 'static,
        T::Response: IntoResponse + 'static,
        T::Future: 'static,
    {
        Self(LocalBoxCloneService::new(MapIntoResponse::new(svc)))
    }

    pub fn oneshot_inner(&self, req: Request) -> RouteFuture<E> {
        let method = req.method().clone();
        RouteFuture::new(method, self.0.clone().oneshot(req))
    }

    pub fn oneshot_inner_owned(self, req: Request) -> RouteFuture<E> {
        let method = req.method().clone();
        RouteFuture::new(method, self.0.oneshot(req))
    }

    pub fn layer<L, E2>(self, layer: L) -> Route<E2>
    where
        L: Layer<Self> + 'static,
        L::Service: TowerService<Request> + Clone + 'static,
        <L::Service as TowerService<Request>>::Response: IntoResponse + 'static,
        <L::Service as TowerService<Request>>::Error: Into<E2> + 'static,
        <L::Service as TowerService<Request>>::Future: 'static,
        E2: 'static,
    {
        let layer = (MapErrLayer::new(Into::into), layer);

        Route::new(layer.layer(self))
    }
}

impl<E> Clone for Route<E> {
    #[track_caller]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<E> fmt::Debug for Route<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Route").finish()
    }
}
