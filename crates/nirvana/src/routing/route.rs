use super::route_tower::{LocalBoxCloneService, MapIntoResponse, RouteFuture};
use crate::prelude::*;
use std::convert::Infallible;
use tower::ServiceExt;

pub struct Route<E = Infallible>(LocalBoxCloneService<Request, Response, E>);

impl<E> Clone for Route<E> {
    #[track_caller]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

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
}
