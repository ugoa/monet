// pub mod handler;
pub mod serve;

use bytes::Bytes;
use http::Method;
use std::{collections::HashMap, pin::Pin};

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

pub struct Body(Pin<Box<dyn http_body::Body<Data = Bytes, Error = BoxError>>>);

pub type Request = http::Request<Body>;
pub type Response = http::Response<Body>;

pub trait Handler {
    fn call(
        &self,
        req: &mut Request,
        resp: &mut Response,
    ) -> Pin<Box<dyn Future<Output = ()> + '_>>;
}

pub(crate) struct RouteIndex(usize);

pub struct Router {
    pub routes: Vec<Route>,
    pub map: matchit::Router<RouteIndex>,
    pub index_to_path: HashMap<RouteIndex, String>,
    pub path_to_index: HashMap<String, RouteIndex>,
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
            map: Default::default(),
            index_to_path: Default::default(),
            path_to_index: Default::default(),
        }
    }

    pub fn at(mut self, path: &str) -> Self {
        self
    }
}
