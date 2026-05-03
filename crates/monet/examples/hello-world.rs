use std::{
    cell::{LazyCell, RefCell},
    net::SocketAddr,
    rc::Rc,
    sync::{Arc, LazyLock, Mutex},
};

use http::header::HeaderValue;
use monet::{Chain, Middleware, Request, Response, Router, async_trait, get};

async fn simple_middleware(req: Request, chain: Chain) -> Response {
    // req.extensions_mut().insert(Rc::new(21));
    let mut resp = chain.next(req).await;
    resp.headers_mut()
        .insert("mark", HeaderValue::from_static("modified"));
    resp
}

static NUM: LazyLock<Arc<Mutex<i32>>> = LazyLock::new(|| Arc::new(Mutex::new(42)));

async fn set_state(mut req: Request, chain: Chain) -> Response {
    let s = &*NUM;
    req.extensions_mut().insert(s.clone());

    let resp = chain.next(req).await;
    resp
}

async fn sample(_req: Request) -> String {
    compio::runtime::time::sleep(std::time::Duration::from_millis(1000)).await;
    let guard = _req.extensions().get::<Arc<Mutex<i32>>>().unwrap();
    let mut state = guard.lock().unwrap();
    *state += 1;
    format!("Hi count is {}", *state)
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
    async fn transform(&self, req: Request, chain: Chain) -> Response {
        COUNTER.with(|inner| *inner.borrow_mut() += 1);
        println!("Count: {}", COUNTER.with(|inner| *inner.borrow()));
        let mut resp = chain.next(req).await;
        resp.headers_mut()
            .insert("count", COUNTER.with(|inner| *inner.borrow()).into());
        resp
    }
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    let app = Router::new()
        .at("/", get(sample))
        .wrap(simple_middleware)
        .at("/hello", get(sample2))
        .wrap(RequestCounter)
        .wrap(set_state);

    monet::serve(addr, app);
}
