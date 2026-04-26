// pub mod handler;
pub mod serve;

use std::{
    cell::{Cell, LazyCell},
    collections::HashMap,
    pin::Pin,
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
    pub index_to_path: HashMap<usize, String>,
    pub path_to_index: HashMap<String, usize>,
}

pub struct Route {
    pub handlers: HashMap<Method, Box<dyn Handler>>,
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
            index_to_path: Default::default(),
            path_to_index: Default::default(),
        }
    }

    pub fn at(mut self, path: &str) -> Self {
        if !self.path_to_index.contains_key(path) {
            let new_index = self.routes.len();
            self.inner
                .insert(path, new_index)
                .expect("should add new path successfully");

            self.routes.push(Route {
                handlers: HashMap::from([(Method::GET, Box::new(DefaultOk) as Box<dyn Handler>)]),
            });
        }
        self
    }

    pub fn merge_for_path(&mut self, path: &str) {}
}
