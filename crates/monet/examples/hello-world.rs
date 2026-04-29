use std::{
    cell::{LazyCell, RefCell},
    net::SocketAddr,
    rc::Rc,
};

thread_local! {
    static COUNTER: LazyCell<RefCell<i32>> = LazyCell::new(|| RefCell::new(0));
}

use futures::stream::Count;
use http::header::HeaderValue;
use monet::{Chain, Request, Response, Router, get, handler};

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

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    COUNTER.with(|inner| *inner.borrow_mut() += 2);
    println!(
        "Running http server from sub crate on {}, count: {}",
        addr,
        COUNTER.with(|inner| inner.borrow().clone())
    );

    let app = Router::new()
        .at("/", get(sample))
        .at("/hello", get(sample2));
    monet::serve(addr, app);
}
