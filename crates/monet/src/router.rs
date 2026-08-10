#[cfg(test)]
pub(crate) mod tests;

pub(crate) mod url;

use core::panic;
use std::{
    cell::RefCell,
    collections::{HashMap, hash_map::Entry},
    path::Path,
    rc::Rc,
    sync::Arc,
};

use http::Method;

use crate::{
    NEVEL_FAIL, ServeDir,
    handler::{Endpoint, Middleware, middleware::strip_prefix::StripPrefix},
    request::Request,
    response::Response,
    router::url::{NEST_TAIL_PARAM, insert_matched_params},
};

type RouteId = usize;

#[derive(Default, Debug)]
pub struct Router {
    matcher: matchit::Router<RouteId>,
    id_to_path: HashMap<RouteId, Rc<str>>,
    routes: Vec<Route>,
    fallback: Option<Rc<dyn Endpoint>>,
}

#[derive(Debug)]
pub enum Route {
    MethodRoute(MethodRoute),
    Service(Layers),
}

#[derive(Default, Debug)]
pub struct MethodRoute {
    pub map: HashMap<Method, Layers>,
    pub fallback: Option<Rc<dyn Endpoint>>,
}

#[derive(Debug, Clone)]
pub struct Layers(Rc<RefCell<SharedLayers>>);

#[derive(Debug, Clone)]
pub struct SharedLayers {
    pub endpoint: Rc<dyn Endpoint>,
    pub middlewares: Vec<Rc<dyn Middleware>>,
}

#[derive(Debug, Clone)]
pub struct Chain {
    handlers: Rc<RefCell<SharedLayers>>,
    cursor: isize,
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

pub fn any(endpoint: impl Endpoint) -> Route {
    let mut mr = MethodRoute::new();
    mr.fallback(endpoint);
    Route::MethodRoute(mr)
}

fn on(endpoint: impl Endpoint, method: Method) -> Route {
    let mut mr = MethodRoute::new();
    mr.register(endpoint, method);

    Route::MethodRoute(mr)
}

impl Router {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn dispatch(&self, mut req: Request) -> impl Future<Output = Response> {
        let path = req.uri().path().to_string();

        let Ok(matched) = self.matcher.at(path.as_str()) else {
            match &self.fallback {
                Some(fallback_handler) => return fallback_handler.call(req),
                None => panic!("Path {} not found", path),
            }
        };
        let route_id: RouteId = *matched.value;

        insert_matched_params(&mut req.state, &matched.params);

        #[cfg(feature = "matched-path")]
        insert_matched_path(
            &mut req.state,
            self.id_to_path.get(&route_id).expect("path shall exist"),
        );

        let route = self.routes.get(route_id).expect(NEVEL_FAIL);

        let method = req.method();

        let resp_fut = match route {
            Route::Service(layers) => Chain::from(layers).next(req),

            Route::MethodRoute(mr) => match mr.map.get(method) {
                Some(layers) => Chain::from(layers).next(req),

                None => {
                    // If no handler for HEAD, try handler for GET instead
                    if method == Method::HEAD
                        && let Some(layers) = mr.map.get(&Method::GET)
                    {
                        Chain::from(layers).next(req)
                    } else {
                        // TODO: Add allowed method in 405 response
                        match &mr.fallback {
                            Some(handler) => return handler.call(req),
                            None => panic!("No handler for `{}` at Path `{}`", method, path),
                        }
                    }
                }
            },
        };

        Box::pin(resp_fut)
    }

    pub fn at(mut self, path: &str, other: Route) -> Self {
        // find() is O(n) operation, but acceptable because it only runs during launching period
        if let Some((route_id, _)) = self.id_to_path.iter().find(|&(_, v)| **v == *path) {
            self.routes
                .get_mut(*route_id)
                .expect("To never fail when Router::add_route() done correctly")
                .merge(other)
        } else {
            self.add_route(path, other)
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

        for (id, route) in other.routes.into_iter().enumerate() {
            let path = other.id_to_path.get(&id).expect(NEVEL_FAIL);

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

        for (id, route) in other.routes.into_iter().enumerate() {
            let inner_path = other.id_to_path.get(&id).expect(NEVEL_FAIL);

            let new_path = concat_path(prefix, inner_path);
            self = self.at(&new_path, route);
        }

        self
    }

    pub fn serve_dir(self, path: &str, dir: impl AsRef<Path>) -> Self {
        let wildcard_path = format!("{}/{{*{}}}", path.trim_end_matches('/'), NEST_TAIL_PARAM);

        let mut layers = Layers::new(ServeDir::new(dir));
        let stripe_prefix_middleware = Rc::new(StripPrefix(Arc::new(path.to_string())));
        layers.push(stripe_prefix_middleware);
        self.at(&wildcard_path, Route::Service(layers))
    }

    pub fn wrap_by(mut self, mw: impl Middleware) -> Self {
        let shared: Rc<dyn Middleware> = Rc::new(mw);

        self.routes.iter_mut().for_each(|route| match route {
            Route::MethodRoute(mr) => mr.map.iter_mut().for_each(|(_, layers)| {
                layers.push(Rc::clone(&shared));
            }),
            Route::Service(layers) => {
                layers.push(Rc::clone(&shared));
            }
        });

        self
    }

    pub fn catch(mut self, endpoint: impl Endpoint) -> Self {
        self.fallback = Some(Rc::new(endpoint));
        self
    }

    fn add_route(&mut self, path: &str, route: Route) {
        let new_route_id = self.routes.len();

        self.matcher.insert(path, new_route_id).expect(NEVEL_FAIL);
        self.routes.push(route);
        self.id_to_path.insert(new_route_id, path.into());
    }
}

impl Layers {
    pub(crate) fn new(endpoint: impl Endpoint) -> Self {
        Layers(Rc::new(RefCell::new(SharedLayers {
            endpoint: Rc::new(endpoint),
            middlewares: Default::default(),
        })))
    }

    pub(crate) fn push(&mut self, m: Rc<dyn Middleware>) {
        self.0.borrow_mut().middlewares.push(m.clone());
    }
}

impl Chain {
    pub fn from(layers: &Layers) -> Self {
        Self {
            handlers: Rc::clone(&layers.0),
            cursor: layers.0.borrow().middlewares.len() as isize,
        }
    }

    pub async fn next(mut self, req: Request) -> Response {
        self.cursor -= 1;

        if self.cursor >= 0 {
            let mw: Rc<dyn Middleware> = self
                .handlers
                .borrow()
                .middlewares
                .get(self.cursor as usize)
                .expect("shall have no out-of-bound error")
                .clone();
            mw.transform(req, self).await
        } else {
            let endpoint: Rc<dyn Endpoint> = self.handlers.borrow().endpoint.clone();
            endpoint.call(req).await
        }
    }
}

impl Route {
    pub fn head(self, endpoint: impl Endpoint) -> Self {
        self.register(endpoint, Method::HEAD)
    }

    pub fn get(self, endpoint: impl Endpoint) -> Self {
        self.register(endpoint, Method::GET)
    }

    pub fn post(self, endpoint: impl Endpoint) -> Self {
        self.register(endpoint, Method::POST)
    }

    pub fn put(self, endpoint: impl Endpoint) -> Self {
        self.register(endpoint, Method::PUT)
    }

    pub fn patch(self, endpoint: impl Endpoint) -> Self {
        self.register(endpoint, Method::PATCH)
    }

    pub fn delete(self, endpoint: impl Endpoint) -> Self {
        self.register(endpoint, Method::DELETE)
    }

    pub fn connect(self, endpoint: impl Endpoint) -> Self {
        self.register(endpoint, Method::CONNECT)
    }

    pub fn options(self, endpoint: impl Endpoint) -> Self {
        self.register(endpoint, Method::OPTIONS)
    }

    pub fn trace(self, endpoint: impl Endpoint) -> Self {
        self.register(endpoint, Method::TRACE)
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
            other.map.iter().for_each(|(method, chain)| {
                match this.map.entry(method.clone()) {
                    Entry::Vacant(e) => e.insert(chain.clone()),
                    Entry::Occupied(_) => {
                        panic!("Overlapping route. Cannot add two endpoints that both handle `{method}`")
                    }
                };
            });
        }
    }

    pub fn register(mut self, endpoint: impl Endpoint, method: Method) -> Self {
        if let Route::MethodRoute(ref mut mr) = self {
            mr.register(endpoint, method);
        }
        self
    }

    pub fn any(mut self, endpoint: impl Endpoint) -> Self {
        if let Route::MethodRoute(ref mut mr) = self {
            mr.fallback = Some(Rc::new(endpoint));
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
        match self.map.entry(method.clone()) {
            Entry::Vacant(e) => e.insert(Layers(Rc::new(RefCell::new(SharedLayers {
                endpoint: Rc::new(endpoint),
                middlewares: Default::default(),
            })))),
            Entry::Occupied(_) => {
                panic!(
                    "Overlapping method route. Cannot add two methods that both handle `{method}`"
                )
            }
        };
    }
}

fn concat_path(prefix: &str, rest: &str) -> String {
    debug_assert!(prefix.starts_with('/'));
    debug_assert!(rest.starts_with('/'));

    if prefix.ends_with('/') {
        // If prefix ends with /, Remove all leading '/'s in the rest path
        format!("{prefix}{}", rest.trim_start_matches('/'))
    } else if rest == "/" {
        prefix.to_string()
    } else {
        format!("{prefix}{rest}")
    }
}
