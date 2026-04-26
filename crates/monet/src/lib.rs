#![allow(clippy::all)]
#![allow(warnings)]
// pub mod handler;
pub mod serve;

use std::{
    cell::{Cell, LazyCell, RefCell},
    collections::{HashMap, hash_map::Entry},
    convert::Infallible,
    marker::PhantomData,
    path,
    pin::Pin,
    rc::Rc,
    sync::{Arc, LazyLock},
};

use bytes::Bytes;
use http::{Method, StatusCode, uri};

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

pub struct Body(Pin<Box<dyn http_body::Body<Data = Bytes, Error = BoxError>>>);

// pub type Request = http::Request<Body>;
// pub type Response = http::Response<Body>;

pub use async_trait::async_trait;
use http_body_util::Full;
use hyper::service::Service as HyperService;

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

pub struct Router {
    pub inner: matchit::Router<usize>,
    pub routes: Vec<Route>,
    pub path_to_index: HashMap<Rc<str>, usize>,
    pub index_to_path: HashMap<usize, Rc<str>>,
}

use hyper::{Request as HyperRequest, Response as HyperResponse, body::Incoming as IncomingBody};
use matchit::MatchError;

pub type Request = HyperRequest<IncomingBody>;
pub type Response = HyperResponse<Full<Bytes>>;

impl HyperService<Request> for Router {
    type Response = Response;
    type Error = hyper::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn call(&self, req: Request) -> Self::Future {
        Box::pin(self.run(req))
    }
}

pub struct Route {
    pub path: Rc<str>,
    pub handlers: RefCell<HashMap<Method, Rc<dyn Handler>>>,
}

impl Route {
    pub fn get(&self, handler: impl Handler + 'static) -> &Self {
        match self.handlers.borrow_mut().entry(Method::GET) {
            Entry::Vacant(entry) => entry.insert(Rc::new(handler)),
            Entry::Occupied(_) => panic!(
                "Overlapping method route. Cannot add two method routes that both handle `GET`"
            ),
        };
        self
    }

    pub fn post(&self, handler: impl Handler + 'static) -> &Self {
        match self.handlers.borrow_mut().entry(Method::POST) {
            Entry::Vacant(entry) => entry.insert(Rc::new(handler)),
            Entry::Occupied(_) => panic!(
                "Overlapping method route. Cannot add two method routes that both handle `POST`"
            ),
        };
        self
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

impl Router {
    pub fn new() -> Self {
        Self {
            inner: Default::default(),
            routes: Default::default(),
            path_to_index: Default::default(),
            index_to_path: Default::default(),
        }
    }

    pub fn run(
        &self,
        mut req: Request,
    ) -> impl Future<Output = Result<Response, hyper::Error>> + 'static {
        let method = req.method();
        let path = req.uri().path();
        // let (mut parts, body) = req.into_parts();
        let match_ = self.inner.at(req.uri().path()).unwrap();
        let idx = *match_.value;
        let route = self.routes.get(idx).expect("should be in router");
        let handler = route.handlers.borrow().get(req.method()).unwrap().clone();

        let mut resp = HyperResponse::new(Full::new(Bytes::from("asdf")));

        async move {
            handler.handle(&mut req, &mut resp).await;
            Ok(resp)
        }
    }

    pub fn at(&mut self, path: &str) -> &Route {
        if !self.path_to_index.contains_key(path) {
            self.insert(path);
        }
        self.routes
            .iter()
            .find(|r| *r.path == *path)
            .expect("should succeed")
    }

    fn insert(&mut self, path: &str) {
        let new_index = self.routes.len();
        self.inner
            .insert(path, new_index)
            .expect("should add new path successfully");

        let route = Route {
            path: path.into(),
            // handlers: HashMap::from([(Method::GET, Box::new(DefaultOk) as Box<dyn Handler>)]),
            handlers: Default::default(),
        };

        self.routes.push(route);
        self.path_to_index.insert(path.into(), new_index);
        self.index_to_path.insert(new_index, path.into());
    }
}

#[test]
fn route_initiate() {
    let mut router = Router::new();
    router
        .at("/")
        .get(async || println!("get"))
        .post(async || println!("post"));
}
