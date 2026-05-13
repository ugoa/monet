use std::net::SocketAddr;

use http::StatusCode;
use monet::{Request, Router, get};

async fn hello(_req: Request) -> &'static str {
    "hello"
}

async fn notfound_404(_req: Request) -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "Page not Found")
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    let app = Router::new()
        .at("/hello", get(hello))
        .fallback(notfound_404);

    monet::run(addr, app);
}
