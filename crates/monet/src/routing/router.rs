use crate::handler::Handler;
use crate::handler::HandlerService;
use crate::prelude::*;
use crate::routing::method_router::MethodRouter;
use crate::routing::route_tower_impl::RouteFuture;
use core::panic;
use matchit::MatchError;
use std::rc::Rc;
use std::{collections::HashMap, convert::Infallible};

#[must_use]
#[derive(Clone)]
pub struct Router {
    pub routes: Vec<Endpoint>,
    pub graph: Graph,
    pub default_fallback: bool,
    pub catch_all_fallback: Fallback,
}

impl fmt::Debug for Router {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Router")
            .field("routes", &self.routes)
            .field("graph", &self.graph)
            .field("default_fallback", &self.default_fallback)
            .field("catch_all_fallback", &self.catch_all_fallback)
            .finish()
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct NotFound;

impl Router {
    pub fn new() -> Self {
        Self {
            routes: Default::default(),
            graph: Default::default(),
            default_fallback: true,
            catch_all_fallback: Fallback::Default(Route::new(NotFound)),
        }
    }

    pub fn chain(mut self, path: &str, method_router: MethodRouter) -> Self {
        todo!()
    }

    pub fn route(mut self, path: &str, method_router: MethodRouter) -> Self {
        self.process_route(path, method_router).unwrap();

        self
    }

    fn process_route(&mut self, path: &str, method_router: MethodRouter) -> Result<(), String> {
        if let Some(route_id) = self.graph.path_to_route_id.get(path) {
            if let Some(Endpoint::MethodRouter(prev_method_router)) = self.routes.get(route_id.0) {
                let service = prev_method_router
                    .clone()
                    .merge_for_path(Some(path), method_router)?;
                let service = Endpoint::MethodRouter(service);
                self.routes[route_id.0] = service;
            }
        } else {
            let endpoint = Endpoint::MethodRouter(method_router);
            self.new_route(path, endpoint).unwrap();
        }
        Ok(())
    }

    fn new_route(&mut self, path: &str, endpoint: Endpoint) -> Result<(), String> {
        let id = RouteId(self.routes.len());
        self.set_node(path, id)?;
        self.routes.push(endpoint);
        Ok(())
    }

    fn set_node(&mut self, path: &str, id: RouteId) -> Result<(), String> {
        self.graph
            .insert(path, id)
            .map_err(|err| format!("Invalid route {path:?}: {err}"))
    }

    pub fn merge<R>(self, other: R) -> Self
    where
        R: Into<Self>,
    {
        let mut this = self.clone();
        let other: Self = other.into();

        let default_fallback = match (this.default_fallback, other.default_fallback) {
            (_, true) => this.default_fallback,
            (true, false) => false,

            (false, false) => {
                panic!("Cannot merge two `Router`s that both have a fallback");
            }
        };

        let catch_all_fallback = this
            .catch_all_fallback
            .clone()
            .merge(other.catch_all_fallback)
            .unwrap_or_else(|| panic!("Cannot merge two `Router`s that both have a fallback"));

        for (id, route) in other.routes.into_iter().enumerate() {
            let route_id = RouteId(id);
            let path = other
                .graph
                .route_id_to_path
                .get(&route_id)
                .expect("no path for route id. This is a bug in axum. Please file an issue");

            match route {
                Endpoint::MethodRouter(method_router) => {
                    this.process_route(path, method_router).unwrap()
                }
                Endpoint::Route(route) => this.new_route(path, Endpoint::Route(route)).unwrap(),
            }
        }
        Router {
            routes: this.routes,
            graph: this.graph,
            default_fallback: default_fallback,
            catch_all_fallback: catch_all_fallback,
        }
    }

    pub fn fallback<H, T>(mut self, handler: H) -> Self
    where
        H: Handler<T>,
        T: 'static,
    {
        self.catch_all_fallback = Fallback::Default(Route::new(HandlerService::new(handler)));
        // Fallback::BoxedHandler(BoxedIntoRoute::from_handler(handler.clone()));
        self
    }

    // pub fn layer<L>(mut self, layer: L) -> Self
    // where
    //     L: TowerLayer<Route> + Clone + 'static,
    //     L::Service: TowerService<HttpRequest> + Clone + 'static,
    //     <L::Service as TowerService<HttpRequest>>::Response: IntoResponse + 'static,
    //     <L::Service as TowerService<HttpRequest>>::Error: Into<Infallible> + 'static,
    //     <L::Service as TowerService<HttpRequest>>::Future: 'static,
    // {
    //     self.routes = self
    //         .routes
    //         .into_iter()
    //         .map(|endpoint| endpoint.layer(layer.clone()))
    //         .collect();
    //
    //     self.catch_all_fallback = self.catch_all_fallback.map(|route| route.layer(layer));
    //     self
    // }

    // pub fn route_layer<L>(mut self, layer: L) -> Self
    // where
    //     L: TowerLayer<Route> + Clone + 'static,
    //     L::Service: TowerService<HttpRequest> + Clone + 'static,
    //     <L::Service as TowerService<HttpRequest>>::Response: IntoResponse + 'static,
    //     <L::Service as TowerService<HttpRequest>>::Error: Into<Infallible> + 'static,
    //     <L::Service as TowerService<HttpRequest>>::Future: 'static,
    // {
    //     self.routes = self
    //         .routes
    //         .into_iter()
    //         .map(|endpoint| endpoint.layer(layer.clone()))
    //         .collect();
    //     self
    // }

    pub(crate) fn call_with_state(&self, req: HttpRequest) -> RouteFuture<Infallible> {
        let (mut parts, body) = req.into_parts();

        println!("{:?}", &self);

        match self.graph.at(parts.uri.path()) {
            Ok(matched) => {
                let route_id = matched.value;

                let endpoint = self.routes.get(route_id.0).expect(
                    "It is granted a valid route for id. Please file an issue if it is not",
                );

                let req = HttpRequest::from_parts(parts, body);

                match endpoint {
                    Endpoint::MethodRouter(method_router) => method_router.call_with_state(req),
                    Endpoint::Route(route) => route.clone().call_owned(req),
                }
            }
            Err(MatchError::NotFound) => {
                let req = HttpRequest::from_parts(parts, body);
                self.catch_all_fallback.clone().call_with_state(req)
            }
        }
    }
}

#[allow(clippy::large_enum_variant)]
pub enum Endpoint {
    MethodRouter(MethodRouter),
    Route(Route),
}

impl fmt::Debug for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MethodRouter(method_router) => {
                f.debug_tuple("MethodRouter").field(method_router).finish()
            }
            Self::Route(route) => f.debug_tuple("Route").field(route).finish(),
        }
    }
}

// impl Endpoint {
//     pub fn layer<L>(self, layer: L) -> Self
//     where
//         L: TowerLayer<Route> + Clone + 'static,
//         L::Service: TowerService<HttpRequest> + Clone + 'static,
//         <L::Service as TowerService<HttpRequest>>::Response: IntoResponse + 'static,
//         <L::Service as TowerService<HttpRequest>>::Error: Into<Infallible> + 'static,
//         <L::Service as TowerService<HttpRequest>>::Future: 'static,
//     {
//         match self {
//             Self::Route(route) => Self::Route(route.layer(layer)),
//             Self::MethodRouter(method_router) => Self::MethodRouter(method_router.layer(layer)),
//         }
//     }
// }

impl Clone for Endpoint {
    fn clone(&self) -> Self {
        match self {
            Self::MethodRouter(inner) => Self::MethodRouter(inner.clone()),
            Self::Route(inner) => Self::Route(inner.clone()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RouteId(pub usize);

#[derive(Clone, Default)]
pub struct Graph {
    pub inner: matchit::Router<RouteId>,
    pub route_id_to_path: HashMap<RouteId, String>,
    pub path_to_route_id: HashMap<String, RouteId>,
}

impl fmt::Debug for Graph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Node")
            .field("paths", &self.route_id_to_path)
            .finish()
    }
}

impl Graph {
    pub fn insert(
        &mut self,
        path: impl Into<String>,
        val: RouteId,
    ) -> Result<(), matchit::InsertError> {
        let path = path.into();

        self.inner.insert(&path, val)?;

        self.route_id_to_path.insert(val, path.clone());
        self.path_to_route_id.insert(path, val);

        Ok(())
    }

    pub fn at<'n, 'p>(
        &'n self,
        path: &'p str,
    ) -> Result<matchit::Match<'n, 'p, &'n RouteId>, MatchError> {
        self.inner.at(path)
    }
}

pub(crate) enum Fallback<E = Infallible> {
    Default(Route<E>),
    Service(Route<E>),
}

impl<E> Clone for Fallback<E> {
    fn clone(&self) -> Self {
        match self {
            Self::Default(inner) => Self::Default(inner.clone()),
            Self::Service(inner) => Self::Service(inner.clone()),
        }
    }
}
impl<E> fmt::Debug for Fallback<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default(inner) => f.debug_tuple("Default").field(inner).finish(),
            Self::Service(inner) => f.debug_tuple("Service").field(inner).finish(),
        }
    }
}

impl<E> Fallback<E> {
    pub fn merge(self, other: Self) -> Option<Self> {
        match (self, other) {
            // If either are `Default`, return the other one, otherwise return None
            (Self::Default(_), pick) => Some(pick),
            (pick, Self::Default(_)) => Some(pick),
            _ => None,
        }
    }

    pub fn map<F, E2>(self, f: F) -> Fallback<E2>
    where
        E: 'static,
        F: FnOnce(Route<E>) -> Route<E2> + Clone + 'static,
        E2: 'static,
    {
        match self {
            Self::Default(route) => Fallback::Default(f(route)),
            Self::Service(route) => Fallback::Service(f(route)),
        }
    }

    pub fn call_with_state(self, req: HttpRequest) -> RouteFuture<E> {
        match self {
            Self::Default(route) | Self::Service(route) => route.oneshot_inner(req),
        }
    }
}
