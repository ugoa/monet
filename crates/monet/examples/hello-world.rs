use std::{
    cell::{LazyCell, RefCell},
    net::SocketAddr,
    sync::{Arc, LazyLock, Mutex},
};

use http::header::HeaderValue;
use monet::{
    Chain, Middleware, Response, Router, async_trait, get, json::Json, post, request::Request,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct UserPayload {
    pub email: String,
    pub password: String,
}

async fn simple_middleware(req: Request, chain: Chain) -> Response {
    // req.extensions_mut().insert(Rc::new(21));
    let mut resp = chain.next(req).await;
    resp.headers_mut()
        .insert("mark", HeaderValue::from_static("modified"));
    resp
}

#[derive(Clone, Debug)]
pub struct SyncedState(i32);

static NUM: LazyLock<Arc<Mutex<SyncedState>>> =
    LazyLock::new(|| Arc::new(Mutex::new(SyncedState(42))));

async fn set_state(mut req: Request, chain: Chain) -> Response {
    let s = &*NUM;
    req.state.insert(s.clone());
    req.state.insert::<SyncedState>(SyncedState(99));

    chain.next(req).await
}

async fn sample(_req: Request) -> String {
    compio::runtime::time::sleep(std::time::Duration::from_millis(1000)).await;
    // let guard = _req.state::<Arc<Mutex<SyncedState>>>().unwrap();
    let guard: &Arc<Mutex<SyncedState>> = _req.state.get().unwrap();
    let mut i = guard.lock().unwrap();
    i.0 += 1;
    format!(
        "Hi count is {}, static number is {}",
        i.0,
        _req.state.get::<SyncedState>().unwrap().0
    )
}

async fn parse_json(req: Request) -> Json<UserPayload> {
    let js: Json<UserPayload> = req.into_json().await.unwrap();
    js
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
        .at("/json", post(parse_json))
        .wrap(RequestCounter)
        .wrap(set_state);

    monet::serve(addr, app);
}
