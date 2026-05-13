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
    handler::{Chain, Endpoint, Middleware, middleware::strip_prefix::StripPrefix},
    request::Request,
    response::Response,
    router::url::{NEST_TAIL_PARAM, concat_path, insert_matched_params, insert_matched_path},
};

pub fn get(handler: impl Endpoint) -> Route {
    let mut md = MethodDispatch::new();
    md.register(handler, Method::GET);

    Route::MethodDispatch(md)
}

pub fn post(handler: impl Endpoint) -> Route {
    let mut md = MethodDispatch::new();
    md.register(handler, Method::POST);

    Route::MethodDispatch(md)
}

pub fn catch(handler: impl Endpoint) -> Route {
    let mut md = MethodDispatch::new();
    md.fallback(handler);

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
            Route::MethodDispatch(dispatcher) => match dispatcher.inner.get(method) {
                Some(chain) => chain.clone().next(req),
                None => match &dispatcher.fallback {
                    Some(handler) => return handler.call(req),
                    None => panic!("No handler for {} Method at Route {}", method, request_path),
                },
            },
        };

        Box::pin(resp_fut)
    }

    pub fn at(mut self, path: &str, route: Route) -> Self {
        if !self.path_to_index.contains_key(path) {
            self.new_route(path, route);
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

    pub fn serve_dir(self, path: &str, dir: impl AsRef<Path>) -> Self {
        let wildcard_path = format!("{}/{{*{}}}", path.trim_end_matches('/'), NEST_TAIL_PARAM);

        let mut chain = Chain::new(ServeDir::new(dir));
        let stripe_prefix_middleware = Rc::new(StripPrefix(Arc::new(path.to_string())));
        chain.append(stripe_prefix_middleware);
        self.at(&wildcard_path, Route::Service(chain))
    }

    pub fn wrap_by(mut self, middleware: impl Middleware) -> Self {
        let shared = Rc::new(middleware);
        self.routes
            .iter_mut()
            .for_each(|route| route.wrap_by(shared.clone()));

        self
    }

    pub fn catch_all(mut self, h: impl Endpoint) -> Self {
        self.fallback = Some(Rc::new(h));
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

#[derive(Default, Debug, Clone)]
pub struct MethodDispatch {
    pub inner: HashMap<Method, Chain>,
    pub fallback: Option<Rc<dyn Endpoint>>,
}

#[derive(Debug, Clone)]
pub enum Route {
    MethodDispatch(MethodDispatch),
    Service(Chain),
}

impl Route {
    pub fn get(self, h: impl Endpoint) -> Self {
        self.register(h, Method::POST)
    }

    pub fn post(self, h: impl Endpoint) -> Self {
        self.register(h, Method::POST)
    }

    pub fn wrap_by(&mut self, middleware: Rc<impl Middleware>) {
        match self {
            Route::MethodDispatch(dispatcher) => {
                dispatcher
                    .inner
                    .iter_mut()
                    .for_each(|(_, chain)| chain.append(middleware.clone()));
            }
            Route::Service(chain) => chain.append(middleware.clone()),
        }
    }

    pub fn register(mut self, h: impl Endpoint, m: Method) -> Self {
        if let Route::MethodDispatch(ref mut dispatcher) = self {
            dispatcher.register(h, m);
        }
        self
    }

    pub fn catch(mut self, h: impl Endpoint) -> Self {
        if let Route::MethodDispatch(ref mut dispatcher) = self {
            dispatcher.fallback = Some(Rc::new(h));
        }
        self
    }
}

impl MethodDispatch {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn fallback(&mut self, h: impl Endpoint) {
        self.fallback = Some(Rc::new(h));
    }

    fn register(&mut self, h: impl Endpoint, m: Method) {
        let chain = Chain {
            endpoint: Rc::new(h),
            middlewares: Default::default(),
        };
        match self.inner.entry(m.clone()) {
            Entry::Vacant(e) => e.insert(chain),
            Entry::Occupied(_) => {
                panic!("Overlapping method route. Cannot add two methods that both handle `{m}`")
            }
        };
    }
}
