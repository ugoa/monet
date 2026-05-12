use std::{
    collections::{HashMap, hash_map::Entry},
    convert::Infallible,
    path::Path,
    rc::Rc,
    sync::Arc,
};

use futures_util::FutureExt;
use http::{Extensions, Method};
use tracing::trace;

pub(crate) mod url;

use crate::{
    ServeDir,
    handler::{Chain, Endpoint, Middleware, middleware::strip_prefix::StripPrefix},
    request::Request,
    response::Response,
    router::url::{NEST_TAIL_PARAM, NEST_TAIL_PARAM_WILDCARD, insert_matched_params},
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
}

impl Router {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn handle(&self, mut req: Request) -> impl Future<Output = Result<Response, Infallible>> {
        let request_path = req.uri().path().to_string();

        let Ok(matched) = self.inner.at(request_path.as_str()) else {
            // TODO:
            //      Return 404 not found if no matching routes, given default-fallback is enabled
            panic!("Path {} not found", request_path);
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

#[derive(Clone, Debug)]
struct MatchedNestedPath(Arc<str>);

#[derive(Clone, Debug)]
pub struct MatchedPath(pub(crate) Arc<str>);

fn insert_matched_path(ext: &mut Extensions, path: &Arc<str>) {
    let matched_path = append_nested_matched_path(&Arc::new(path), ext);

    if matched_path.ends_with(NEST_TAIL_PARAM_WILDCARD) {
        ext.insert(MatchedNestedPath(matched_path));
        debug_assert!(ext.remove::<MatchedPath>().is_none());
    } else {
        ext.insert(MatchedPath(matched_path));
        ext.remove::<MatchedNestedPath>();
    }
}

fn append_nested_matched_path(matched_path: &Arc<str>, extensions: &http::Extensions) -> Arc<str> {
    if let Some(previous) = extensions
        .get::<MatchedPath>()
        .map(|matched_path| &matched_path.0)
        .or_else(|| Some(&extensions.get::<MatchedNestedPath>()?.0))
    {
        let previous = previous
            .strip_suffix(NEST_TAIL_PARAM_WILDCARD)
            .unwrap_or(previous);

        let matched_path = format!("{previous}{matched_path}");
        matched_path.into()
    } else {
        Arc::clone(matched_path)
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
