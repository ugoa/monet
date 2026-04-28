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

#[derive(Default)]
pub struct Route(RefCell<HashMap<Method, Rc<dyn Handler>>>);

pub fn get(handler: impl Handler + 'static) -> Route {
    Route::new().get(handler)
}

pub fn post(handler: impl Handler + 'static) -> Route {
    Route::new().post(handler)
}

pub fn patch(handler: impl Handler + 'static) -> Route {
    Route::new().patch(handler)
}
pub fn put(handler: impl Handler + 'static) -> Route {
    Route::new().put(handler)
}
pub fn delete(handler: impl Handler + 'static) -> Route {
    Route::new().delete(handler)
}
pub fn connect(handler: impl Handler + 'static) -> Route {
    Route::new().connect(handler)
}
pub fn options(handler: impl Handler + 'static) -> Route {
    Route::new().options(handler)
}
pub fn trace(handler: impl Handler + 'static) -> Route {
    Route::new().trace(handler)
}
pub fn head(handler: impl Handler + 'static) -> Route {
    Route::new().head(handler)
}

impl Route {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn get(mut self, h: impl Handler + 'static) -> Self {
        self.register(h, Method::GET)
    }

    pub fn post(mut self, h: impl Handler + 'static) -> Self {
        self.register(h, Method::POST)
    }

    pub fn patch(mut self, h: impl Handler + 'static) -> Self {
        self.register(h, Method::PATCH)
    }

    pub fn put(mut self, h: impl Handler + 'static) -> Self {
        self.register(h, Method::PUT)
    }

    pub fn delete(mut self, h: impl Handler + 'static) -> Self {
        self.register(h, Method::DELETE)
    }

    pub fn connect(mut self, h: impl Handler + 'static) -> Self {
        self.register(h, Method::CONNECT)
    }

    pub fn options(mut self, h: impl Handler + 'static) -> Self {
        self.register(h, Method::OPTIONS)
    }

    pub fn trace(mut self, h: impl Handler + 'static) -> Self {
        self.register(h, Method::TRACE)
    }

    pub fn head(mut self, h: impl Handler + 'static) -> Self {
        self.register(h, Method::HEAD)
    }

    fn register(mut self, h: impl Handler + 'static, m: Method) -> Self {
        match self.0.borrow_mut().entry(m.clone()) {
            Entry::Vacant(e) => e.insert(Rc::new(h)),
            Entry::Occupied(_) => {
                panic!("Overlapping method route. Cannot add two methods that both handle `{m}`",)
            }
        };
        self
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MethodHandler {
    method: Method,
    handler: Rc<dyn Handler>,
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
        // TODO: Return 404 not found if no matching routes, given default-fallback is enabled
        let match_ = self.inner.at(req.uri().path()).unwrap();
        let idx = *match_.value;
        let route = self.routes.get(idx).expect("should be in router");
        // TODO: Return 404 not found if no matching method, given default-fallback is enabled
        let handler = route.0.borrow().get(req.method()).unwrap().clone();

        let mut resp = HyperResponse::new(Full::new(Bytes::from("original")));

        async move {
            compio::runtime::time::sleep(std::time::Duration::from_millis(2000)).await;
            handler.handle(&mut req, &mut resp).await;
            Ok(resp)
        }
    }

    pub fn at(mut self, path: &str, route: Route) -> Router {
        if !self.path_to_index.contains_key(path) {
            self.new_path(path, route);
        }
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
