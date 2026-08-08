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
    handler::{Chain, Endpoint, Middleware, middleware::strip_prefix::StripPrefix},
    request::Request,
    response::Response,
    router::url::{NEST_TAIL_PARAM, insert_matched_params, insert_matched_path},
};

pub fn any(endpoint: impl Endpoint) -> Route {
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

type RouteId = usize;

#[derive(Default, Debug)]
pub struct Router {
    matcher: matchit::Router<RouteId>,
    id_to_path: HashMap<RouteId, Rc<str>>,
    routes: Vec<Route>,
    fallback: Option<Rc<dyn Endpoint>>,
}

impl Router {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn dispatch(&self, mut req: Request) -> impl Future<Output = Response> {
        let path = req.uri().path().to_string();

        let Ok(matched) = self.matcher.at(path.as_str()) else {
            match &self.fallback {
                Some(handler) => return handler.call(req),
                None => panic!("Path {} not found", path),
            }
        };
        let route_id: RouteId = *matched.value;

        insert_matched_params(&mut req.state, &matched.params);

        #[cfg(not(feature = "no-matched-path"))]
        insert_matched_path(
            &mut req.state,
            self.id_to_path.get(&route_id).expect("path shall exist"),
        );

        let route = self.routes.get(route_id).expect(NEVEL_FAIL);

        let method = req.method();
        let resp_fut = match route {
            Route::Service(svc) => svc.clone().next(req),
            Route::MethodRoute(mr) => match mr.inner.get(method) {
                /*
                 * Tradeoff: Given a chain with M middlewares and 1 endpoint, A total words of
                 *    M (middleware Rc) +
                 *    3 (The Vec itself) +
                 *    1 (endpoint Rc)
                 * are being allocated by the .clone() per request. We could've use slice of Vec
                 * same as the tide framework, but this would pollute the Middleware API with
                 * lifetime annotation. This is a performance tradeoff in favor of the DX simplicity.
                 */
                Some(chain) => chain.clone().next(req),
                None => {
                    // If no handler for HEAD method, try handler for GET instead
                    if method == Method::HEAD
                        && let Some(chain) = mr.inner.get(&Method::GET)
                    {
                        chain.clone().next(req)
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
                .expect("To never fail, as route must be present at this path")
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

        let mut chain = Chain::new(ServeDir::new(dir));
        let stripe_prefix_middleware = Rc::new(StripPrefix(Arc::new(path.to_string())));
        chain.append(stripe_prefix_middleware);
        self.at(&wildcard_path, Route::Service(chain))
    }

    pub fn wrap_by(mut self, mw: impl Middleware) -> Self {
        let shared = Rc::new(mw);
        self.routes
            .iter_mut()
            .for_each(|route| route.wrap_by(Rc::clone(&shared)));

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

#[derive(Debug)]
pub enum Route {
    MethodRoute(MethodRoute),
    Service(Chain),
}

#[derive(Default, Debug)]
pub struct MethodRoute {
    pub inner: HashMap<Method, Chain>,
    pub fallback: Option<Rc<dyn Endpoint>>,
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
            other.inner.iter().for_each(|(method, chain)| {
                match this.inner.entry(method.clone()) {
                    Entry::Vacant(e) => e.insert(chain.clone()),
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
                .for_each(|(_, chain)| chain.append(Rc::clone(&mw))),
            Route::Service(chain) => chain.append(Rc::clone(&mw)),
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
        match self.inner.entry(method.clone()) {
            Entry::Vacant(e) => e.insert(Chain {
                endpoint: Rc::new(endpoint),
                middlewares: Default::default(),
            }),
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
