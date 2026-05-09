use std::{
    collections::{HashMap, hash_map::Entry},
    convert::Infallible,
    pin::Pin,
    rc::Rc,
};

use futures_util::FutureExt;
use http::Method;

pub fn get(handler: impl Endpoint) -> Route {
    Route::new().get(handler)
}

pub fn post(handler: impl Endpoint) -> Route {
    Route::new().post(handler)
}

use crate::{
    handler::{Chain, Endpoint, Middleware},
    request::Request,
    response::{IntoResponse, Response},
};

#[derive(Default)]
pub struct Router {
    pub inner: matchit::Router<usize>,
    pub routes: Vec<Route>,
    pub middlewares: Rc<Vec<Rc<dyn Middleware>>>,
    pub path_to_index: HashMap<Rc<str>, usize>,
    pub index_to_path: HashMap<usize, Rc<str>>,
}

#[derive(Default)]
pub struct Route(pub HashMap<Method, Chain>);

impl Route {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn get(mut self, h: impl Endpoint) -> Self {
        self.register(h, Method::GET)
    }

    pub fn post(mut self, h: impl Endpoint) -> Self {
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

    pub fn run(
        &self,
        mut req: Request,
    ) -> impl Future<Output = Result<Response, Infallible>> + 'static {
        let method = req.method();
        let path = req.uri().path();
        // TODO:
        //      Return 404 not found if no matching routes, given default-fallback is enabled
        let match_ = self.inner.at(req.uri().path()).unwrap();
        let idx = *match_.value;
        let route = self.routes.get(idx).expect("should be in router");
        // TODO:
        //      Return 404 not found if no matching method, given default-fallback is enabled
        let chain = route.0.get(req.method()).unwrap().clone();

        chain.next(req).map(|x| Ok::<_, Infallible>(x))
    }

    pub fn at(mut self, path: &str, route: Route) -> Self {
        if !self.path_to_index.contains_key(path) {
            self.new_path(path, route);
        }
        self
    }

    pub fn wrap(mut self, middleware: impl Middleware) -> Self {
        let shared = Rc::new(middleware);
        self.routes.iter_mut().for_each(|route| {
            route
                .0
                .iter_mut()
                .for_each(|(_, chain)| chain.middlewares.push(shared.clone()));
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
