use crate::handler::{Handler, HandlerService};
use crate::prelude::*;
use crate::routing::method_filter::MethodFilter;
use crate::routing::route::{BoxedIntoRoute, ErasedIntoRoute, Route};
use crate::routing::route_tower_impl::RouteFuture;
use crate::routing::router::Fallback;
use http::{Method, StatusCode};
use std::collections::HashMap;
use std::convert::Infallible;
use tower::{Layer, service_fn};

pub fn get<H, X>(handler: H) -> MethodRouter<Infallible>
where
    H: Handler<X>,
    X: 'static,
{
    MethodRouter::new().get(handler)
}

pub fn post<H, X>(handler: H) -> MethodRouter<Infallible>
where
    H: Handler<X>,
    X: 'static,
{
    MethodRouter::new().post(handler)
}

#[derive(Clone, Debug)]
pub struct MethodRouter<E = Infallible> {
    mapping: HashMap<Method, Route<E>>,
    fallback: Fallback<E>,
}

impl<E> MethodRouter<E> {
    pub fn call_with_state(&self, req: HttpRequest) -> RouteFuture<E> {
        todo!()
    }
}

impl MethodRouter {
    pub fn get<H, X>(mut self, handler: H) -> Self
    where
        H: Handler<X>,
        X: 'static,
    {
        self.on(Method::GET, handler)
    }

    pub fn post<H, X>(mut self, handler: H) -> Self
    where
        H: Handler<X>,
        X: 'static,
    {
        self.on(Method::POST, handler)
    }

    pub fn on<H, X>(mut self, method: Method, handler: H) -> Self
    where
        H: Handler<X>,
        X: 'static,
    {
        let route = Route::new(HandlerService::new(handler));
        if self.mapping.contains_key(&method) {
            panic!("Overlapping method route. Cannot add two routes that both handle `{method}`");
        } else {
            self.mapping.insert(method, route);
        }

        self
    }

    pub(crate) fn merge_for_path(&self, path: Option<&str>, method_router: MethodRouter) -> Self {
        todo!()
    }
}

impl<E> MethodRouter<E> {
    pub fn new() -> Self {
        let fallback = Route::new(service_fn(|_: HttpRequest| async {
            Ok(StatusCode::METHOD_NOT_ALLOWED)
        }));
        Self {
            mapping: HashMap::default(),
            fallback: Fallback::Default(fallback),
        }
    }
}
