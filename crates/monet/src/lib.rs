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
use http::{Method, StatusCode};

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
    pub handlers: RefCell<HashMap<Method, Box<dyn Handler>>>,
}

impl Route {
    fn get(&self, handler: impl Handler + 'static) -> &Self {
        match self.handlers.borrow_mut().entry(Method::GET) {
            Entry::Vacant(entry) => entry.insert(Box::new(handler)),
            Entry::Occupied(_) => panic!(
                "Overlapping method route. Cannot add two method routes that both handle `GET`"
            ),
        };
        self
    }

    fn post(&self, handler: impl Handler + 'static) -> &Self {
        match self.handlers.borrow_mut().entry(Method::POST) {
            Entry::Vacant(entry) => entry.insert(Box::new(handler)),
            Entry::Occupied(_) => panic!(
                "Overlapping method route. Cannot add two method routes that both handle `POST`"
            ),
        };
        self
    }
}

// impl Router {
//     fn new() -> Self {
//         Self {
//             inner: Default::default(),
//         }
//     }
// }

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
        fn mk_response(s: String) -> Result<Response, hyper::Error> {
            Ok(HyperResponse::new(Full::new(Bytes::from(s))))
        }
        let res = match req.uri().path() {
            "/" => mk_response(format!("home! counter")),
            "/posts" => mk_response(format!("posts, of course! counter")),
            _ => mk_response("oh no! not found".into()),
        };
        async { res }
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

async fn get_handler() {
    println!("should get")
}

async fn post_handler() {
    println!("should post")
}

#[test]
fn route_initiate() {
    let mut router = Router::new();
    router.at("/").get(get_handler).post(post_handler);
}
