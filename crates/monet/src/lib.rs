#![allow(clippy::all)]
#![allow(warnings)]
// pub mod handler;
pub mod serve;

use std::{
    cell::{Cell, LazyCell, RefCell},
    collections::{HashMap, VecDeque, hash_map::Entry},
    convert::Infallible,
    marker::PhantomData,
    path,
    pin::Pin,
    process::Output,
    rc::Rc,
    sync::{Arc, LazyLock},
};

use bytes::Bytes;
use futures::FutureExt;
use http::{HeaderValue, Method, StatusCode, uri};

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

pub struct Body(Pin<Box<dyn http_body::Body<Data = Bytes, Error = BoxError>>>);

#[async_trait(?Send)]
pub trait Middleware: 'static {
    async fn transform(&self, request: Request, chain: Chain) -> Response;

    /// Set the middleware's name. By default it uses the type signature.
    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }
}

#[async_trait(?Send)]
impl<F, Fut> Middleware for F
where
    F: 'static + Fn(Request, Chain) -> Fut,
    Fut: Future<Output = Response>,
{
    async fn transform(&self, req: Request, chain: Chain) -> Response {
        (self)(req, chain).await
    }
}

#[async_trait(?Send)]
pub trait Endpoint: 'static {
    async fn call(&self, req: Request) -> Response;
}

#[async_trait(?Send)]
impl<F, Fut, Resp> Endpoint for F
where
    F: 'static + Fn(Request) -> Fut,
    Fut: Future<Output = Resp>,
    Resp: IntoResponse,
{
    async fn call(&self, req: Request) -> Response {
        (self)(req).await.into_response()
    }
}

pub trait IntoResponse {
    #[must_use]
    fn into_response(self) -> Response;
}

impl IntoResponse for String {
    fn into_response(self) -> Response {
        Response::new(Full::new(Bytes::from(self)))
    }
}

impl IntoResponse for &'static str {
    fn into_response(self) -> Response {
        Response::new(Full::new(Bytes::from(self)))
    }
}

#[derive(Clone)]
pub struct Chain {
    pub(crate) endpoint: Rc<dyn Endpoint>,
    pub(crate) middlewares: Vec<Rc<dyn Middleware>>,
}

impl Chain {
    pub async fn call_next(mut self, req: Request) -> Response {
        if let Some(current) = self.middlewares.pop() {
            current.transform(req, self).await
        } else {
            self.endpoint.call(req).await
        }
    }
}

// pub type Request = http::Request<Body>;
// pub type Response = http::Response<Body>;
pub use async_trait::async_trait;
use http_body_util::Full;
use hyper::service::Service as HyperService;
pub use monet_macros::handler;
pub use serve::serve;

#[async_trait(?Send)]
pub trait Handler {
    async fn handle(&self, req: &mut Request, resp: &mut Response);
}

#[async_trait(?Send)]
impl<F, Fut> Handler for F
where
    F: FnMut() -> Fut + Clone,
    Fut: Future<Output = ()>,
{
    async fn handle(&self, req: &mut Request, resp: &mut Response) {
        self.clone()();
    }
}

struct DefaultOk;
#[async_trait(?Send)]
impl Handler for DefaultOk {
    async fn handle(&self, _req: &mut Request, resp: &mut Response) {
        *resp.status_mut() = StatusCode::OK;
    }
}

use hyper::{Request as HyperRequest, Response as HyperResponse, body::Incoming as IncomingBody};
use matchit::MatchError;

pub type Request = HyperRequest<IncomingBody>;
pub type Response = HyperResponse<Full<Bytes>>;

#[derive(Default)]
pub struct Router {
    pub inner: matchit::Router<usize>,
    pub routes: Vec<Route>,
    pub middlewares: Rc<Vec<Rc<dyn Middleware>>>,
    pub path_to_index: HashMap<Rc<str>, usize>,
    pub index_to_path: HashMap<usize, Rc<str>>,
}

impl HyperService<Request> for Router {
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn call(&self, req: Request) -> Self::Future {
        Box::pin(self.run(req))
    }
}

#[derive(Default)]
pub struct Route(HashMap<Method, Chain>);

pub fn get(handler: impl Endpoint) -> Route {
    Route::new().get(handler)
}

pub fn post(handler: impl Endpoint) -> Route {
    Route::new().post(handler)
}

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

        chain.call_next(req).map(|x| Ok::<_, Infallible>(x))
    }

    pub fn at(mut self, path: &str, route: Route) -> Self {
        if !self.path_to_index.contains_key(path) {
            self.new_path(path, route);
        }
        self
    }

    pub fn wrap_with(mut self, middleware: impl Middleware) -> Self {
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
