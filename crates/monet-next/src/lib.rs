// pub mod handler;
pub mod serve;

use bytes::Bytes;
use std::{collections::HashMap, pin::Pin};

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

pub struct Body(Pin<Box<dyn http_body::Body<Data = Bytes, Error = BoxError>>>);

pub type Request = http::Request<Body>;
pub type Response = http::Response<Body>;

pub(crate) trait Handler {
    fn call(
        &self,
        req: &mut Request,
        resp: &mut Response,
    ) -> Pin<Box<dyn Future<Output = ()> + '_>>;
}

pub struct Router {
    pub routes: Vec<Endpoint>,
    pub node: Node,
}
pub struct Endpoint {
    get: Option<Box<dyn Handler>>,
    head: Option<Box<dyn Handler>>,
    delete: Option<Box<dyn Handler>>,
    options: Option<Box<dyn Handler>>,
    patch: Option<Box<dyn Handler>>,
    post: Option<Box<dyn Handler>>,
    put: Option<Box<dyn Handler>>,
    trace: Option<Box<dyn Handler>>,
    connect: Option<Box<dyn Handler>>,
}

#[derive(Default)]
pub struct Node {
    pub inner: matchit::Router<usize>,
    pub id_to_path: HashMap<usize, String>,
    pub path_to_id: HashMap<String, usize>,
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
            node: Default::default(),
        }
    }
}
