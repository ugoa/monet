#[cfg(test)]
pub(crate) mod tests;

pub(crate) mod url;

use core::panic;
use std::{
    collections::{HashMap, hash_map::Entry},
    path::Path,
    rc::Rc,
    sync::Arc,
};

use http::Method;

use crate::{
    NEVEL_FAIL, ServeDir,
    handler::{Endpoint, Layer, Middleware, middleware::strip_prefix::StripPrefix},
    request::Request,
    response::Response,
    router::url::{NEST_TAIL_PARAM, concat_path, insert_matched_params2, insert_matched_path},
};

pub fn catch(endpoint: impl Endpoint) -> Route {
    let mut mr = MethodRoute::new();
    mr.fallback(endpoint);
    Route::MethodRoute(mr)
}

pub fn get(endpoint: impl Endpoint) -> Route {
    on(endpoint, Method::GET)
}

pub fn post(endpoint: impl Endpoint) -> Route {
    on(endpoint, Method::POST)
}

pub fn connect(endpoint: impl Endpoint) -> Route {
    on(endpoint, Method::CONNECT)
}

pub fn head(endpoint: impl Endpoint) -> Route {
    on(endpoint, Method::HEAD)
}

pub fn put(endpoint: impl Endpoint) -> Route {
    on(endpoint, Method::PUT)
}

pub fn patch(endpoint: impl Endpoint) -> Route {
    on(endpoint, Method::PATCH)
}

pub fn delete(endpoint: impl Endpoint) -> Route {
    on(endpoint, Method::DELETE)
}

pub fn trace(endpoint: impl Endpoint) -> Route {
    on(endpoint, Method::TRACE)
}

pub fn options(endpoint: impl Endpoint) -> Route {
    on(endpoint, Method::OPTIONS)
}

fn on(endpoint: impl Endpoint, method: Method) -> Route {
    let mut mr = MethodRoute::new();
    mr.register(endpoint, method);

    Route::MethodRoute(mr)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct RouteId(usize);

#[derive(Default, Debug)]
pub struct Router {
    pub route_matcher: matchit::Router<RouteId>,
    pub routes: Vec<Route>,
    pub path_to_index: HashMap<Arc<str>, RouteId>, // TODO: change to Rc
    pub index_to_path: HashMap<RouteId, Arc<str>>,
    pub fallback: Option<Rc<dyn Endpoint>>,
}

impl Router {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn dispatch(&self, mut req: Request) -> impl Future<Output = Response> {
        let request_path = req.uri().path().to_string();

        let Ok(matched) = self.route_matcher.at(request_path.as_str()) else {
            match &self.fallback {
                Some(handler) => return handler.call(req),
                None => panic!("Path {} not found", request_path),
            }
        };

        let index: RouteId = *matched.value;

        // let ext_mut = req.extensions_mut();

        // #[cfg(not(feature = "no-matched-path"))]
        // insert_matched_path(ext_mut, self.index_to_path.get(&index).unwrap());

        insert_matched_params2(&mut req.state, &matched.params);

        // dbg!(&matched.params);

        let route = self.routes.get(index.0).expect(NEVEL_FAIL);

        let method = req.method();
        let resp_fut = match route {
            Route::Service(svc) => svc.clone().next(req),
            Route::MethodRoute(method_route) => match method_route.inner.get(method) {
                /*
                 * Tradeoff: Given a layer with M middlewares and 1 endpoint, A total words of
                 *    M (middleware Rc) +
                 *    3 (The Vec itself) +
                 *    1 (endpoint Rc)
                 * are being allocated by the .clone() per request. We could've use slice of Vec
                 * as the tide framework does, but this would pollute the Middleware interface with
                 * lifetime annotation. This is a performance tradeoff in favor of the DX simplicity.
                 */
                Some(layer) => layer.clone().next(req),
                None => match &method_route.fallback {
                    Some(handler) => return handler.call(req),
                    None => panic!("No handler for {} Method at Route {}", method, request_path),
                },
            },
        };

        Box::pin(resp_fut)
    }

    pub fn at(mut self, path: &str, other: Route) -> Self {
        match self.path_to_index.get(path) {
            Some(route_id) => self.routes.get_mut(route_id.0).unwrap().merge(other),
            None => self.new_route(path, other),
        }
        self
    }

    pub fn merge(mut self, other: Self) -> Self {
        // Merge fallback
        match (&self.fallback, &other.fallback) {
            (Some(f), None) | (None, Some(f)) => self.fallback = Some(Rc::clone(f)),
            (None, None) => (),
            (Some(_), Some(_)) => {
                panic!("Cannot merge two `Router`s that both have a fallback")
            }
        }

        for (index, route) in other.routes.into_iter().enumerate() {
            let path = other.index_to_path.get(&RouteId(index)).expect(NEVEL_FAIL);

            self = self.at(path, route);
        }
        self
    }

    pub fn nest(mut self, prefix: &str, other: Self) -> Self {
        assert!(prefix.starts_with('/'));
        assert!(prefix.len() > 1);

        if prefix
            .split('/')
            .any(|seg| seg.starts_with("{*") && seg.ends_with('}') && !seg.ends_with("}}"))
        {
            panic!("Invalid route: nested routes cannot contain wildcards (*)");
        }

        for (index, route) in other.routes.into_iter().enumerate() {
            let inner_path = other.index_to_path.get(&RouteId(index)).expect(NEVEL_FAIL);

            let new_path = concat_path(prefix, inner_path);
            self = self.at(&new_path, route);
        }

        self
    }

    pub fn serve_dir(self, path: &str, dir: impl AsRef<Path>) -> Self {
        let wildcard_path = format!("{}/{{*{}}}", path.trim_end_matches('/'), NEST_TAIL_PARAM);

        let mut layer = Layer::new(ServeDir::new(dir));
        let stripe_prefix_middleware = Rc::new(StripPrefix(Arc::new(path.to_string())));
        layer.append(stripe_prefix_middleware);
        self.at(&wildcard_path, Route::Service(layer))
    }

    pub fn wrap_by(mut self, mw: impl Middleware) -> Self {
        let shared = Rc::new(mw);
        self.routes
            .iter_mut()
            .for_each(|route| route.wrap_by(Rc::clone(&shared)));

        self
    }

    pub fn catch_all(mut self, endpoint: impl Endpoint) -> Self {
        self.fallback = Some(Rc::new(endpoint));
        self
    }

    fn new_route(&mut self, path: &str, route: Route) {
        let new_index = self.routes.len();
        self.route_matcher
            .insert(path, RouteId(new_index))
            .expect(NEVEL_FAIL);

        self.routes.push(route);
        self.path_to_index.insert(path.into(), RouteId(new_index));
        self.index_to_path.insert(RouteId(new_index), path.into());
    }
}

#[derive(Debug)]
pub enum Route {
    MethodRoute(MethodRoute),
    Service(Layer),
}

#[derive(Default, Debug)]
pub struct MethodRoute {
    pub inner: HashMap<Method, Layer>,
    pub fallback: Option<Rc<dyn Endpoint>>,
}

impl Route {
    pub fn get(self, endpoint: impl Endpoint) -> Self {
        self.register(endpoint, Method::POST)
    }

    pub fn post(self, endpoint: impl Endpoint) -> Self {
        self.register(endpoint, Method::POST)
    }

    pub fn merge(&mut self, other: Route) {
        if let &mut Route::MethodRoute(ref mut this) = self
            && let Route::MethodRoute(ref other) = other
        {
            match (&this.fallback, &other.fallback) {
                (Some(f), None) | (None, Some(f)) => this.fallback = Some(Rc::clone(f)),
                (Some(_), Some(_)) => {
                    panic!("Cannot merge two `Route`s of same path that both have a fallback")
                }
                (None, None) => (),
            }
            other.inner.iter().for_each(|(method, layer)| {
                match this.inner.entry(method.clone()) {
                    Entry::Vacant(e) => e.insert(layer.clone()),
                    Entry::Occupied(_) => {
                        panic!("Overlapping route. Cannot add two endpoints that both handle `{method}`")
                    }
                };
            });
        }
    }

    pub fn wrap_by(&mut self, mw: Rc<impl Middleware>) {
        match self {
            Route::MethodRoute(mr) => mr
                .inner
                .iter_mut()
                .for_each(|(_, layer)| layer.append(Rc::clone(&mw))),
            Route::Service(layer) => layer.append(Rc::clone(&mw)),
        }
    }

    pub fn register(mut self, endpoint: impl Endpoint, method: Method) -> Self {
        if let Route::MethodRoute(ref mut dispatch) = self {
            dispatch.register(endpoint, method);
        }
        self
    }

    pub fn catch(mut self, endpoint: impl Endpoint) -> Self {
        if let Route::MethodRoute(ref mut dispatch) = self {
            dispatch.fallback = Some(Rc::new(endpoint));
        }
        self
    }
}

impl MethodRoute {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn fallback(&mut self, endpoint: impl Endpoint) {
        self.fallback = Some(Rc::new(endpoint));
    }

    fn register(&mut self, endpoint: impl Endpoint, method: Method) {
        let layer = Layer {
            endpoint: Rc::new(endpoint),
            middlewares: Default::default(),
        };
        match self.inner.entry(method.clone()) {
            Entry::Vacant(e) => e.insert(layer),
            Entry::Occupied(_) => {
                panic!(
                    "Overlapping method route. Cannot add two methods that both handle `{method}`"
                )
            }
        };
    }
}
