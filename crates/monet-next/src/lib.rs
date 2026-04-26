// pub mod handler;
pub mod serve;

use bytes::Bytes;
use http::{Method, StatusCode};
use std::{
    cell::{Cell, LazyCell},
    collections::HashMap,
    pin::Pin,
    sync::{Arc, LazyLock},
};

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
    pub routes: Vec<Route>,
    pub graph: matchit::Router<usize>,
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
            routes: Default::default(),
            graph: Default::default(),
            index_to_path: Default::default(),
            path_to_index: Default::default(),
        }
    }

    pub fn at(mut self, path: &str) -> Self {
        if let Some(idx) = self.path_to_index.get(path) {
            if let Some(route) = self.routes.get(*idx) {
                self.merge_for_path(path)
            }
        } else {
            let new_index = self.routes.len();
            let expection = "should add new path successfully";
            self.graph.insert(path, new_index).expect(expection);

            let default_handler: Box<dyn Handler> = Box::new(DefaultOk);
            self.routes.push(Route {
                handlers: HashMap::from([(Method::GET, default_handler)]),
            });
        }
        self
    }

    pub fn merge_for_path(&mut self, path: &str) {}
}
