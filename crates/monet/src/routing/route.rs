use super::route_tower_impl::{LocalBoxCloneService, MapIntoResponse, RouteFuture};
use crate::{
    handler::{Handler, HandlerService},
    prelude::*,
};
use std::convert::Infallible;
use tower::{util::MapErrLayer, ServiceExt};

pub struct Route<'a, E = Infallible>(LocalBoxCloneService<'a, HttpRequest, HttpResponse, E>);

impl<'a, E> Route<'a, E> {
    pub fn new<T>(svc: T) -> Self
    where
        T: TowerService<HttpRequest, Error = E> + Clone + 'a,
        T::Response: IntoResponse + 'a,
        T::Future: 'a,
    {
        Self(LocalBoxCloneService::new(MapIntoResponse::new(svc)))
    }

    /// Variant of [`Route::call`] that takes ownership of the route to avoid cloning.
    pub(crate) fn call_owned(self, req: HttpRequest<Body>) -> RouteFuture<'a, E> {
        self.oneshot_inner(req.map(Body::new))
    }

    pub fn oneshot_inner(&self, req: HttpRequest) -> RouteFuture<'a, E> {
        let method = req.method().clone();
        RouteFuture::new(method, self.0.clone().oneshot(req))
    }

    /// Variant of [`Route::oneshot_inner`] that takes ownership of the route to avoid cloning.
    pub(crate) fn oneshot_inner_owned(self, req: HttpRequest) -> RouteFuture<'a, E> {
        let method = req.method().clone();
        RouteFuture::new(method, self.0.oneshot(req))
    }

    // pub fn layer<L, E2>(self, layer: L) -> Route<'a, E2>
    // where
    //     L: TowerLayer<Self> + 'static,
    //     L::Service: TowerService<HttpRequest> + Clone + 'a,
    //     <L::Service as TowerService<HttpRequest>>::Response: IntoResponse + 'a,
    //     <L::Service as TowerService<HttpRequest>>::Error: Into<E2> + 'a,
    //     <L::Service as TowerService<HttpRequest>>::Future: 'a,
    //     E2: 'static,
    // {
    //     let layer = (MapErrLayer::new(Into::into), layer);
    //
    //     Route::new(layer.layer(self))
    // }
}

impl<'a, E> Clone for Route<'a, E> {
    #[track_caller]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<'a, E> fmt::Debug for Route<'a, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Route").finish()
    }
}

pub(crate) struct BoxedIntoRoute<'a, S, E>(pub Box<dyn ErasedIntoRoute<'a, S, E> + 'a>);

pub(crate) trait ErasedIntoRoute<'a, S, E> {
    fn clone_box(&self) -> Box<dyn ErasedIntoRoute<'a, S, E> + 'a>;

    fn into_route(self: Box<Self>, state: S) -> Route<'a, E>;
}

impl<'a, S, E> Clone for BoxedIntoRoute<'a, S, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

///  Transfer Layer Map to Route
// pub(crate) struct Map<S, E, E2> {
//     pub(crate) inner: Box<dyn ErasedIntoRoute<S, E>>,
//     pub(crate) layer: Box<dyn LayerFn<E, E2>>,
// }

pub(crate) trait LayerFn<'a, E, E2>: FnOnce(Route<'a, E>) -> Route<'a, E2> {
    fn clone_box(&self) -> Box<dyn LayerFn<'a, E, E2> + 'a>;
}

impl<'a, F, E, E2> LayerFn<'a, E, E2> for F
where
    F: FnOnce(Route<'a, E>) -> Route<'a, E2> + Clone + 'a,
{
    fn clone_box(&self) -> Box<dyn LayerFn<'a, E, E2> + 'a> {
        Box::new(self.clone())
    }
}

// impl<S, E, E2> ErasedIntoRoute<S, E2> for Map<S, E, E2>
// where
//     S: 'static,
//     E: 'static,
//     E2: 'static,
// {
//     fn clone_box(&self) -> Box<dyn ErasedIntoRoute<S, E2>> {
//         Box::new(Self {
//             inner: self.inner.clone_box(),
//             layer: self.layer.clone_box(),
//         })
//     }
//
//     fn into_route(self: Box<Self>, state: S) -> Route<E2> {
//         (self.layer)(self.inner.into_route(state))
//     }
// }

impl<'a, S, E> BoxedIntoRoute<'a, S, E> {
    // pub(crate) fn map<F, E2>(self, f: F) -> BoxedIntoRoute<S, E2>
    // where
    //     S: 'static,
    //     E: 'static,
    //     F: FnOnce(Route<E>) -> Route<E2> + Clone + 'static,
    //     E2: 'static,
    // {
    //     BoxedIntoRoute(Box::new(Map {
    //         inner: self.0,
    //         layer: Box::new(f),
    //     }))
    // }

    pub(crate) fn into_route(self, state: S) -> Route<'a, E> {
        self.0.into_route(state)
    }
}

///  Transfer handler to Route
impl<'a, S> BoxedIntoRoute<'a, S, Infallible>
where
    S: Clone + 'a,
{
    pub fn from_handler<H, X>(handler: H) -> Self
    where
        H: Handler<'a, X, S> + 'a,
        X: 'a,
    {
        let svc_fn = |handler, state| {
            let svc = HandlerService::new(handler, state);
            let resp_map = MapIntoResponse::new(svc);
            let lbcs = LocalBoxCloneService::new(resp_map);
            Route(lbcs)
        };
        let erased = ErasedHandler {
            handler: handler,
            into_route_fn: svc_fn,
        };
        BoxedIntoRoute(Box::new(erased))
    }
}

/// This struct stores 2 function pointers:
/// 1. The handler function itself
/// 2. A function that turns handler w/ state into a Route
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
    H: Clone,
{
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            into_route_fn: self.into_route_fn,
        }
    }
}
