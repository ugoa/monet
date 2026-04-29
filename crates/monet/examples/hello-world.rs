use std::{
    cell::{LazyCell, RefCell},
    net::SocketAddr,
    rc::Rc,
};

use futures::stream::Count;
use http::header::HeaderValue;
use monet::{Chain, Middleware, Request, Response, Router, async_trait, get, handler};
use tracing::info;

#[handler]
async fn omni_api(resp: &mut Response) {
    resp.headers_mut()
        .insert("mark", HeaderValue::from_static("modified"));
}

async fn sample(_req: Request) -> String {
    "Hi".to_string()
}

async fn sample2(_req: Request) -> &'static str {
    "Hello"
}

thread_local! {
    static COUNTER: LazyCell<RefCell<i32>> = LazyCell::new(|| RefCell::new(0));
}

struct RequestCount;

#[async_trait(?Send)]
impl Middleware for RequestCount {
    async fn transform(&self, req: Request, chain: Chain) -> Response {
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
        .at("/hello", get(sample2));
    monet::serve(addr, app);
}
