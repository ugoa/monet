pub(crate) mod url;

use std::{
    collections::{HashMap, hash_map::Entry},
    path::Path,
    rc::Rc,
    sync::Arc,
};

use http::Method;

use crate::{
    ServeDir,
    handler::{Chain, Endpoint, Middleware, middleware::strip_prefix::StripPrefix},
    request::Request,
    response::Response,
    router::url::{NEST_TAIL_PARAM, concat_path, insert_matched_params, insert_matched_path},
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
    pub path_to_index: HashMap<Arc<str>, usize>, // TODO: change to Rc
    pub index_to_path: HashMap<usize, Arc<str>>,
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

        let id = *matched.value;

        let ext_mut = req.extensions_mut();

        // #[cfg(not(feature = "no-matched-path"))]
        insert_matched_path(ext_mut, self.index_to_path.get(&id).unwrap());

        insert_matched_params(ext_mut, &matched.params);

        // dbg!(&matched.params);

        let route = self.routes.get(id).expect("should be in router");

        let method = req.method();
        let resp_fut = match route {
            Route::Service(svc) => svc.clone().next(req),
            Route::MethodGraph(map) => {
                let chain = map.0.get(method).expect("handler should exist").clone();
                chain.next(req)
            }
        };

        Box::pin(resp_fut)
    }

    pub fn at(mut self, path: &str, route: Route) -> Self {
        if !self.path_to_index.contains_key(path) {
            self.create(path, route);
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
            let assertion =
                "The path should've been registered already, otherwise please report a bug";
            let inner_path = other.index_to_path.get(&id).expect(assertion);

            let new_path = concat_path(prefix, inner_path);
            self = self.at(&new_path, route);
        }

        self
    }

    pub fn merge(mut self, other: Self) -> Self {
        for (id, route) in other.routes.into_iter().enumerate() {
            let assertion =
                "The path should've been registered already, otherwise please report a bug";
            let path = other.index_to_path.get(&id).expect(assertion);

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

    pub fn fallback(mut self, h: impl Endpoint) -> Self {
        self.fallback = Some(Rc::new(h));
        self
    }

    fn create(&mut self, path: &str, route: Route) {
        let new_index = self.routes.len();
        self.inner
            .insert(path, new_index)
            .expect("should add new path successfully");

        self.routes.push(route);
        self.path_to_index.insert(path.into(), new_index);
        self.index_to_path.insert(new_index, path.into());
    }
}

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
