use std::net::SocketAddr;

use http::StatusCode;
use monet::{Request, Router, get, router::fallback};

async fn hello(_req: Request) -> &'static str {
    "hello"
}

async fn partial_support(_req: Request) -> &'static str {
    "Only GET is supported at this route"
}

async fn no_support(_req: Request) -> &'static str {
    "No support at this route"
}

async fn global_notfound(_req: Request) -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "Page not Found")
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    let app = Router::new()
        .at("/hi", fallback(no_support))
        .at("/hello", get(hello).fallback(partial_support))
        .catch_all(global_notfound);

    monet::run(addr, app);
}
