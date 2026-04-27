#![allow(clippy::all)]
#![allow(warnings)]
pub mod macros;
pub mod serve;

use std::{
    cell::{Cell, LazyCell, RefCell},
    collections::{HashMap, hash_map::Entry},
    convert::Infallible,
    marker::PhantomData,
    path,
    pin::Pin,
    process::Output,
    rc::Rc,
    sync::{Arc, LazyLock},
};

use bytes::Bytes;
use http::{HeaderValue, Method, StatusCode, uri};

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

// #[async_trait(?Send)]
// impl<F, Fut> Handler for F
// where
//     F: FnMut(&mut Response) -> Fut + Clone,
//     Fut: Future<Output = ()>,
// {
//     fn handle<'life0, 'life1, 'life2, 'async_trait>(
//         &'life0 self,
//         __macro_gen_req: &'life1 mut Request,
//         resp: &'life2 mut Response,
//     ) -> Pin<Box<dyn Future<Output = ()> + 'async_trait>>
//     where
//         'life0: 'async_trait,
//         'life1: 'async_trait,
//         'life2: 'async_trait,
//         Self: 'async_trait,
//     {
//         Box::pin(async move { self.clone()(resp).await })
//     }
// }

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

    // pub fn post(&self, handler: impl Handler + 'static) -> &Self {
    //     match self.handlers.borrow_mut().entry(Method::POST) {
    //         Entry::Vacant(entry) => entry.insert(Rc::new(handler)),
    //         Entry::Occupied(_) => panic!(
    //             "Overlapping method route. Cannot add two method routes that both handle `POST`"
    //         ),
    //     };
    //     self
    // }
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

        let mut resp = HyperResponse::new(Full::new(Bytes::from("original")));

        async move {
            compio::runtime::time::sleep(std::time::Duration::from_millis(2000)).await;
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

async fn get_handler(resp: &mut Response) {
    resp.headers_mut()
        .insert("mark", HeaderValue::from_static("modified"));
}

async fn simple_hello() {
    "he";
}

#[test]
fn route_initiate() {
    let mut router = Router::new();
    router.at("/").get(simple_hello);
}
