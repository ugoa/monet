use std::{
    collections::{HashMap, hash_map::Entry},
    convert::Infallible,
    path::Path,
    process::Child,
    rc::Rc,
    sync::Arc,
};

use futures_util::FutureExt;
use http::Method;
use tracing::trace;

use crate::{
    ServeDir,
    handler::{Chain, Endpoint, Middleware, middleware::strip_prefix::StripPrefix},
    request::Request,
    response::Response,
};

pub fn get(handler: impl Endpoint) -> Route {
    Route::MethodGraph(MethodGraph::new().get(handler))
}

pub fn post(handler: impl Endpoint) -> Route {
    Route::MethodGraph(MethodGraph::new().post(handler))
}

#[derive(Default, Debug)]
pub struct Router {
    pub inner: matchit::Router<usize>,
    pub routes: Vec<Route>,
    pub middlewares: Rc<Vec<Rc<dyn Middleware>>>,
    pub path_to_index: HashMap<Rc<str>, usize>,
    pub index_to_path: HashMap<usize, Rc<str>>,
}

impl Router {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn handle(&self, req: Request) -> impl Future<Output = Result<Response, Infallible>> {
        let _method = req.method();
        let _path = req.uri().path();

        // TODO:
        //      Return 404 not found if no matching routes, given default-fallback is enabled
        let match_ = self.inner.at(_path).unwrap();
        let idx = *match_.value;
        let route = self.routes.get(idx).expect("should be in router");

        let resp_fut = match route {
            Route::Service(svc) => svc.clone().next(req),
            Route::MethodGraph(map) => {
                let chain = map.0.get(_method).expect("handler should exist").clone();
                chain.next(req)
            }
        };

        resp_fut.map(Ok::<_, Infallible>)
    }

    pub fn at(mut self, path: &str, route: Route) -> Self {
        if !self.path_to_index.contains_key(path) {
            self.new_path(path, route);
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

        for (id, route) in other.routes.into_iter().enumerate() {
            let assertion = "should always succeed, otherwise it would be a monet bug";
            let inner_path = other.index_to_path.get(&id).expect(assertion);

            let new_path = concat_path(prefix, inner_path);
            self = self.at(&new_path, route);
        }

        self
    }

    pub fn merge(self, _path: &str, _other: Self) -> Self {
        todo!()
    }

    pub fn serve_dir(self, path: &str, dir: impl AsRef<Path>) -> Self {
        let wildcard_path = format!("{}/{{*{}}}", path.trim_end_matches('/'), NEST_TAIL_PARAM);

        let mut chain = Chain::new(ServeDir::new(dir));
        let stripe_prefix_middleware = Rc::new(StripPrefix(Arc::new(path.to_string())));
        chain.append(stripe_prefix_middleware);
        self.at(&wildcard_path, Route::Service(chain))
    }

    pub fn wrap_by(mut self, middleware: impl Middleware) -> Self {
        trace!("Adding middleware: {}", middleware.name());
        let shared = Rc::new(middleware);
        self.routes
            .iter_mut()
            .for_each(|route| route.wrap_by(shared.clone()));

        self
    }

    fn new_path(&mut self, path: &str, route: Route) {
        let new_index = self.routes.len();
        self.inner
            .insert(path, new_index)
            .expect("should add new path successfully");

        self.routes.push(route);
        self.path_to_index.insert(path.into(), new_index);
        self.index_to_path.insert(new_index, path.into());
    }
}

pub(crate) const NEST_TAIL_PARAM: &str = "__private__monet_nest_tail_param";

#[derive(Default, Debug, Clone)]
pub struct MethodGraph(pub HashMap<Method, Chain>);

#[derive(Debug, Clone)]
pub enum Route {
    MethodGraph(MethodGraph),
    Service(Chain),
}

impl Route {
    pub fn wrap_by(&mut self, middleware: Rc<impl Middleware>) {
        match self {
            Route::MethodGraph(map) => {
                map.0
                    .iter_mut()
                    .for_each(|(_, chain)| chain.append(middleware.clone()));
            }
            Route::Service(chain) => chain.append(middleware.clone()),
        }
    }
}

impl MethodGraph {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn get(self, h: impl Endpoint) -> Self {
        self.register(h, Method::GET)
    }

    pub fn post(self, h: impl Endpoint) -> Self {
        self.register(h, Method::POST)
    }

    fn register(mut self, h: impl Endpoint, m: Method) -> Self {
        let chain = Chain {
            endpoint: Rc::new(h),
            middlewares: Default::default(),
        };
        match self.0.entry(m.clone()) {
            Entry::Vacant(e) => e.insert(chain),
            Entry::Occupied(_) => {
                panic!("Overlapping method route. Cannot add two methods that both handle `{m}`")
            }
        };
        self
    }
}

fn concat_path(prefix: &str, path: &str) -> String {
    debug_assert!(prefix.starts_with('/'));
    debug_assert!(path.starts_with('/'));

    if prefix.ends_with('/') {
        format!("{prefix}{}", path.trim_start_matches('/'))
    } else if path == "/" {
        prefix.into()
    } else {
        format!("{prefix}{path}")
    }
}
