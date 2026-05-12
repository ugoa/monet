use std::{
    cell::{LazyCell, RefCell},
    net::SocketAddr,
    sync::{Arc, LazyLock, Mutex},
};

use http::header::HeaderValue;
use monet::{
    Chain, Form, Json, Middleware, Response, Router, async_trait, error::Error, get,
    handler::endpoint::serve_dir::ServeDir, post, request::Request, types::Html,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct UserPayload {
    pub email: String,
    pub password: String,
}

async fn simple_middleware(req: Request, chain: Chain) -> Response {
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

#[derive(Deserialize)]
pub struct Pagination {
    pub page: i32,
    pub offset: i32,
}

async fn root(req: Request) -> String {
    compio::runtime::time::sleep(std::time::Duration::from_millis(1000)).await;

    // let guard = _req.state::<Arc<Mutex<SyncedState>>>().unwrap();
    let guard: &Arc<Mutex<SyncedState>> = req.state.get().unwrap();
    let mut i = guard.lock().unwrap();
    i.0 += 1;
    format!("Hi count is {}", i.0)
}

async fn query(req: Request) -> Result<String, Error> {
    let q = req.query::<Pagination>()?;
    Ok(q.offset.to_string())
}

async fn parse_json(req: Request) -> Result<Json<UserPayload>, Error> {
    req.into_json().await
}

#[derive(Deserialize, Serialize)]
pub struct FormPayload {
    pub name: String,
    pub email: String,
}
async fn parse_form(req: Request) -> Result<Form<FormPayload>, Error> {
    req.into_form().await
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

    let service = ServeDir::new("static");

    let app = Router::new()
        .at("/", get(root))
        .at("/query", get(query))
        .wrap_by(simple_middleware)
        .at("/json", post(parse_json))
        .at("/form", post(parse_form))
        .at("/html", get(return_html))
        .wrap_by(RequestCounter)
        .wrap_by(set_state)
        .at("/hello.html", get(service));

    monet::run(addr, app);
}
