use std::net::SocketAddr;

use http::StatusCode;
use monet::{Request, Router, get, router::any};

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

    monet::run(addr, || {
        Router::new()
            .at("/hi", any(no_support))
            .at("/hello", get(hello).any(partial_support))
            .catch(global_notfound)
    });
}
