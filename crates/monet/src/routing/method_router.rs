use crate::handler::{Handler, HandlerService};
use crate::prelude::*;
use crate::routing::method_filter::MethodFilter;
use crate::routing::route::Route;
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
    pub(crate) mapping: HashMap<Method, Route<E>>,
    pub(crate) fallback: Fallback<E>,
}

impl<E> MethodRouter<E> {
    pub fn call(&self, req: HttpRequest) -> RouteFuture<E> {
        if *req.method() == Method::HEAD {
            if let Some(route) = self
                .mapping
                .get(&Method::HEAD)
                .or_else(|| self.mapping.get(&Method::GET))
            {
                return route.clone().oneshot_inner_owned(req);
            }
        } else {
            if let Some(route) = self.mapping.get(req.method()) {
                return route.clone().oneshot_inner_owned(req);
            }
        }

        self.fallback.clone().call_with_state(req)
    }

    pub(crate) fn merge_for_path(
        mut self,
        path: Option<&str>,
        other: Self,
    ) -> Result<Self, String> {
        for method in self.mapping.keys() {
            if other.mapping.contains_key(&method) {
                let error_message = if let Some(path) = path {
                    format!(
                        "Overlapping method route. Handler for `{method} {path}` already exists",
                    )
                } else {
                    format!(
                        "Overlapping method route. Cannot merge two method routes that both define `{method}`"
                    )
                };
                return Err(error_message);
            }
        }

        self.mapping.extend(other.mapping);
        self.fallback = self
            .fallback
            .merge(other.fallback)
            .ok_or("Cannot merge two `MethodRouter`s that both have a fallback")?;

        Ok(self)
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
