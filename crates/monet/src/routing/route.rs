use super::route_tower_impl::{LocalBoxCloneService, MapIntoResponse, RouteFuture};
use crate::{
    handler::{Handler, HandlerService},
    prelude::*,
};
use std::convert::Infallible;
use tower::{ServiceExt, util::MapErrLayer};

pub struct Route<E = Infallible>(LocalBoxCloneService<HttpRequest, HttpResponse, E>);

impl<E> Route<E> {
    pub fn new<T>(svc: T) -> Self
    where
        T: TowerService<HttpRequest, Error = E> + Clone + 'static,
        T::Response: IntoResponse + 'static,
        T::Future: 'static,
    {
        Self(LocalBoxCloneService::new(MapIntoResponse::new(svc)))
    }

    /// Variant of [`Route::call`] that takes ownership of the route to avoid cloning.
    pub(crate) fn call_owned(self, req: HttpRequest<Body>) -> RouteFuture<E> {
        self.oneshot_inner(req.map(Body::new))
    }

    pub fn oneshot_inner(&self, req: HttpRequest) -> RouteFuture<E> {
        let method = req.method().clone();
        RouteFuture::new(method, self.0.clone().oneshot(req))
    }

    /// Variant of [`Route::oneshot_inner`] that takes ownership of the route to avoid cloning.
    pub(crate) fn oneshot_inner_owned(self, req: HttpRequest) -> RouteFuture<E> {
        let method = req.method().clone();
        RouteFuture::new(method, self.0.oneshot(req))
    }

    pub fn layer<L, E2>(self, layer: L) -> Route<E2>
    where
        L: TowerLayer<Self> + 'static,
        L::Service: TowerService<HttpRequest> + Clone + 'static,
        <L::Service as TowerService<HttpRequest>>::Response: IntoResponse + 'static,
        <L::Service as TowerService<HttpRequest>>::Error: Into<E2> + 'static,
        <L::Service as TowerService<HttpRequest>>::Future: 'static,
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
