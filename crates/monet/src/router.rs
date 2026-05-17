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
    GUARANTEE, ServeDir,
    handler::{Endpoint, Layer, Middleware, middleware::strip_prefix::StripPrefix},
    request::Request,
    response::Response,
    router::url::{NEST_TAIL_PARAM, concat_path, insert_matched_params, insert_matched_path},
};

pub fn catch(endpoint: impl Endpoint) -> Route {
    let mut md = MethodDispatch::new();
    md.fallback(endpoint);
    Route::MethodDispatch(md)
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
    let mut md = MethodDispatch::new();
    md.register(endpoint, method);

    Route::MethodDispatch(md)
}

#[derive(Default, Debug)]
pub struct Router {
    pub inner: matchit::Router<usize>,
    pub routes: Vec<Route>,
    pub path_to_index: HashMap<Arc<str>, usize>, // TODO: change to Rc
    pub index_to_path: HashMap<usize, Arc<str>>,
    pub middlewares: Rc<Vec<Rc<dyn Middleware>>>,
    pub fallback: Option<Rc<dyn Endpoint>>,
}

impl Router {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn handle(&self, mut req: Request) -> impl Future<Output = Response> {
        let request_path = req.uri().path().to_string();

        let Ok(matched) = self.inner.at(request_path.as_str()) else {
            match &self.fallback {
                Some(handler) => return handler.call(req),
                None => panic!("Path {} not found", request_path),
            }
        };

        let index = *matched.value;

        let ext_mut = req.extensions_mut();

        // #[cfg(not(feature = "no-matched-path"))]
        insert_matched_path(ext_mut, self.index_to_path.get(&index).unwrap());

        insert_matched_params(ext_mut, &matched.params);

        // dbg!(&matched.params);

        let route = self.routes.get(index).expect(GUARANTEE);

        let method = req.method();
        let resp_fut = match route {
            Route::Service(svc) => svc.clone().next(req),
            Route::MethodDispatch(dispatch) => match dispatch.inner.get(method) {
                /*
                 * Tradeoff: Given a layer with M middlewares and 1 endpoint, A total of
                 * M(middleware Rc) + 3(The Vec itself) + 1(endpoint Rc) words(8 bytes of each)
                 * are being allocated by the .clone() per request. We could've use slice of Vec
                 * as the tide framework does, but this would pollute the Middleware interface with
                 * lifetime annotation. This is a performance tradeoff in faver of the DX simplicity.
                 */
                Some(layer) => layer.clone().next(req),
                None => match &dispatch.fallback {
                    Some(handler) => return handler.call(req),
                    None => panic!("No handler for {} Method at Route {}", method, request_path),
                },
            },
        };

        Box::pin(resp_fut)
    }

    pub fn at(mut self, path: &str, other: Route) -> Self {
        match self.path_to_index.get(path) {
            Some(index) => self.routes.get_mut(*index).unwrap().merge(other),
            None => self.new_route(path, other),
        }
        self
    }

    pub fn merge(mut self, other: Self) -> Self {
        // Merge fallback
        match (&self.fallback, &other.fallback) {
            (Some(f), None) | (None, Some(f)) => self.fallback = Some(f.clone()),
            (None, None) => (),
            (Some(_), Some(_)) => {
                panic!("Cannot merge two `Router`s that both have a fallback")
            }
        }

        for (index, route) in other.routes.into_iter().enumerate() {
            let path = other.index_to_path.get(&index).expect(GUARANTEE);

            self = self.at(path, route);
        }
        self
    }

    pub fn nest(mut self, prefix: &str, other: Self) -> Self {
        assert!(prefix.starts_with('/'));
        assert!(prefix.len() > 1);

        if prefix.split('/').any(|segment| {
            segment.starts_with("{*") && segment.ends_with('}') && !segment.ends_with("}}")
        }) {
            panic!("Invalid route: nested routes cannot contain wildcards (*)");
        }

        for (index, route) in other.routes.into_iter().enumerate() {
            let inner_path = other.index_to_path.get(&index).expect(GUARANTEE);

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

    pub fn wrap_by(mut self, middleware: impl Middleware) -> Self {
        let shared = Rc::new(middleware);
        self.routes
            .iter_mut()
            .for_each(|route| route.wrap_by(shared.clone()));

        self
    }

    pub fn catch_all(mut self, endpoint: impl Endpoint) -> Self {
        self.fallback = Some(Rc::new(endpoint));
        self
    }

    fn new_route(&mut self, path: &str, route: Route) {
        let new_index = self.routes.len();
        self.inner.insert(path, new_index).expect(GUARANTEE);

        self.routes.push(route);
        self.path_to_index.insert(path.into(), new_index);
        self.index_to_path.insert(new_index, path.into());
    }
}

#[derive(Debug, Clone)]
pub enum Route {
    MethodDispatch(MethodDispatch),
    Service(Layer),
}

#[derive(Default, Debug, Clone)]
pub struct MethodDispatch {
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
        if let &mut Route::MethodDispatch(ref mut this) = self
            && let Route::MethodDispatch(ref other) = other
        {
            match (&this.fallback, &other.fallback) {
                (Some(f), None) | (None, Some(f)) => this.fallback = Some(f.clone()),
                (None, None) => (),
                (Some(_), Some(_)) => {
                    panic!("Cannot merge two `Route`s of same path that both have a fallback")
                }
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

    pub fn wrap_by(&mut self, middleware: Rc<impl Middleware>) {
        match self {
            Route::MethodDispatch(dispatch) => dispatch
                .inner
                .iter_mut()
                .for_each(|(_, layer)| layer.append(middleware.clone())),
            Route::Service(layer) => layer.append(middleware.clone()),
        }
    }

    pub fn register(mut self, endpoint: impl Endpoint, method: Method) -> Self {
        if let Route::MethodDispatch(ref mut dispatch) = self {
            dispatch.register(endpoint, method);
        }
        self
    }

    pub fn catch(mut self, endpoint: impl Endpoint) -> Self {
        if let Route::MethodDispatch(ref mut dispatch) = self {
            dispatch.fallback = Some(Rc::new(endpoint));
        }
        self
    }
}

impl MethodDispatch {
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
