use std::{
    cell::{LazyCell, RefCell},
    net::SocketAddr,
    rc::Rc,
};

use futures::stream::Count;
use http::header::HeaderValue;
use hyper::service::service_fn;
use monet::{Chain, Middleware, Request, Response, Router, async_trait, get, handler};
use tracing::info;

#[handler]
async fn omni_api2(resp: &mut Response) {
    resp.headers_mut()
        .insert("mark", HeaderValue::from_static("modified"));
}

async fn sample(_req: Request) -> String {
    compio::runtime::time::sleep(std::time::Duration::from_millis(1000)).await;
    "Hi".to_string()
}

async fn sample2(_req: Request) -> &'static str {
    compio::runtime::time::sleep(std::time::Duration::from_millis(1000)).await;
    "Hello"
}

thread_local! {
    static COUNTER: LazyCell<RefCell<i32>> = LazyCell::new(|| RefCell::new(0));
}

struct RequestCounter;

#[async_trait(?Send)]
impl Middleware for RequestCounter {
    async fn transform(&self, req: Request, chain: Chain) -> Result<Response, hyper::Error> {
        COUNTER.with(|inner| *inner.borrow_mut() += 1);
        println!("Count: {}", COUNTER.with(|inner| inner.borrow().clone()));
        chain.call_next(req).await
    }
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    let app = Router::new()
        .at("/", get(sample))
        .at("/hello", get(sample2))
        .wrap_with(RequestCounter);

    monet::serve(addr, app);
}
