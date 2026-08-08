use std::{
    cell::{LazyCell, RefCell},
    net::SocketAddr,
    pin::Pin,
    sync::{Arc, LazyLock, Mutex},
};

use http::header::HeaderValue;
use monet::{Chain, Middleware, Response, Router, get, request::Request, types::Html};

async fn simple_middleware(req: Request, chain: Chain) -> Response {
    let mut resp = chain.next(req).await;
    resp.headers_mut()
        .insert("mark", HeaderValue::from_static("modified"));
    resp
}

#[derive(Clone, Debug)]
pub struct SyncedState(i32);

static NUM: LazyLock<Arc<Mutex<SyncedState>>> =
    LazyLock::new(|| Arc::new(Mutex::new(SyncedState(0))));

async fn set_state(mut req: Request, chain: Chain) -> Response {
    let s = &*NUM;
    req.state.set(s.clone());
    req.state.set::<SyncedState>(SyncedState(99));

    chain.next(req).await
}

async fn root(req: Request) -> String {
    compio::runtime::time::sleep(std::time::Duration::from_millis(1000)).await;

    // let guard = _req.state::<Arc<Mutex<SyncedState>>>().unwrap();
    let guard: &Arc<Mutex<SyncedState>> = req.state.get().unwrap();
    let mut i = guard.lock().unwrap();
    i.0 += 1;
    format!("Hi count is {}", i.0)
}

async fn return_html(_req: Request) -> Html<&'static str> {
    Html(
        r#"
        <!doctype html>
        <html>
            <head>
                <title>Hello from Monet </title>
            </head>
            <body>
                <h3>Welcome!</h3>
            </body>
        </html>
        "#,
    )
}

thread_local! {
    static COUNTER: LazyCell<RefCell<i32>> = LazyCell::new(|| RefCell::new(0));
}

#[derive(Clone)]
struct RequestCounter;

impl Middleware for RequestCounter {
    fn transform(
        &self,
        req: Request,
        chain: Chain,
    ) -> Pin<Box<dyn Future<Output = Response> + '_>> {
        COUNTER.with(|inner| *inner.borrow_mut() += 1);
        println!("Count: {}", COUNTER.with(|inner| *inner.borrow()));

        Box::pin(async move {
            let mut resp = chain.next(req).await;
            resp.headers_mut()
                .insert("count", COUNTER.with(|inner| *inner.borrow()).into());
            resp
        })
    }
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    monet::run(addr, || {
        Router::new()
            .at("/", get(root))
            .wrap_by(simple_middleware)
            .at("/html", get(return_html))
            .wrap_by(RequestCounter)
            .wrap_by(set_state)
    });
}
