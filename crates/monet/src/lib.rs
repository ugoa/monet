#![allow(clippy::all)]
#![allow(warnings)]
// pub mod handler;
pub mod serve;

use std::{
    cell::{Cell, LazyCell},
    collections::HashMap,
    path,
    pin::Pin,
    rc::Rc,
    sync::{Arc, LazyLock},
};

use bytes::Bytes;
use http::{Method, StatusCode};

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

pub struct Body(Pin<Box<dyn http_body::Body<Data = Bytes, Error = BoxError>>>);

pub type Request = http::Request<Body>;
pub type Response = http::Response<Body>;

pub use async_trait::async_trait;

#[async_trait(?Send)]
pub trait Handler {
    async fn call(&self, req: &mut Request, resp: &mut Response);
}

struct DefaultOk;
#[async_trait(?Send)]
impl Handler for DefaultOk {
    async fn call(&self, _req: &mut Request, resp: &mut Response) {
        *resp.status_mut() = StatusCode::OK;
    }
}

pub struct Router {
    pub inner: matchit::Router<usize>,
    pub routes: Vec<Route>,
    pub path_to_index: HashMap<Rc<str>, usize>,
    pub index_to_path: HashMap<usize, Rc<str>>,
}

pub struct Route {
    pub path: Rc<str>,
    pub handlers: HashMap<Method, Box<dyn Handler>>,
}

impl Route {
    fn get(&mut self, handler: impl Handler) -> Self {
        todo!();
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
            handlers: HashMap::from([(Method::GET, Box::new(DefaultOk) as Box<dyn Handler>)]),
        };

        self.routes.push(route);
        self.path_to_index.insert(path.into(), new_index);
        self.index_to_path.insert(new_index, path.into());
    }
}

#[test]
fn route_initiate() {
    let mut router = Router::new();
    let route = router.at("/");
    assert_eq!(&*route.path, "/");
}
