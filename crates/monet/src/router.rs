use std::{
    collections::{HashMap, hash_map::Entry},
    convert::Infallible,
    rc::Rc,
};

use futures_util::FutureExt;
use http::Method;
use tracing::trace;

use crate::{
    handler::{Chain, Endpoint, Middleware},
    request::Request,
    response::Response,
};

pub fn get(handler: impl Endpoint) -> Route {
    Route::MethodGraph(MethodGraph::new().post(handler))
}

pub fn post(handler: impl Endpoint) -> Route {
    Route::MethodGraph(MethodGraph::new().post(handler))
}

pub fn service(handler: impl Endpoint) -> Route {
    Route::Service(Rc::new(handler))
}

#[derive(Default)]
pub struct Router {
    pub inner: matchit::Router<usize>,
    pub routes: Vec<Route>,
    pub middlewares: Rc<Vec<Rc<dyn Middleware>>>,
    pub path_to_index: HashMap<Rc<str>, usize>,
    pub index_to_path: HashMap<usize, Rc<str>>,
}

// #[derive(Default)]
// pub struct Route(pub HashMap<Method, Chain>);

#[derive(Default)]
pub struct MethodGraph(pub HashMap<Method, Chain>);

pub enum Route {
    MethodGraph(MethodGraph),
    Service(Rc<dyn Endpoint>),
}

impl Route {
    fn register(mut self, h: impl Endpoint, m: Method) -> Self {
        let chain = Chain {
            endpoint: Rc::new(h),
            middlewares: Default::default(),
        };

        if let Self::MethodGraph(ref mut map) = self {
            match map.0.entry(m.clone()) {
                Entry::Vacant(e) => e.insert(chain),
                Entry::Occupied(_) => {
                    panic!(
                        "Overlapping method route. Cannot add two methods that both handle `{m}`"
                    )
                }
            };
        } else {
            panic!("Cannot register A service already registered at current path")
        }
        self
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

impl Router {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn run(&self, req: Request) -> impl Future<Output = Result<Response, Infallible>> {
        let _method = req.method();
        let _path = req.uri().path();

        // TODO:
        //      Return 404 not found if no matching routes, given default-fallback is enabled
        let match_ = self.inner.at(_path).unwrap();
        let idx = *match_.value;
        let route = self.routes.get(idx).expect("should be in router");

        let resp_fut = match route {
            Route::Service(svc) => svc.call(req),
            Route::MethodGraph(map) => {
                let chain = map.0.get(req.method()).unwrap().clone();
                Box::pin(chain.next(req))
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

    pub fn nest(self, _path: &str, _other: Self) -> Self {
        todo!()
    }

    pub fn merge(self, _path: &str, _other: Self) -> Self {
        todo!()
    }

    pub fn wrap(mut self, middleware: impl Middleware) -> Self {
        trace!("Adding middleware {}", middleware.name());
        let shared = Rc::new(middleware);
        self.routes.iter_mut().for_each(|route| match route {
            Route::MethodGraph(map) => {
                map.0
                    .iter_mut()
                    .for_each(|(_, chain)| chain.middlewares.push(shared.clone()));
            }
            Route::Service(_) => (),
        });

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
