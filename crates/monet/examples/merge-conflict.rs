use std::net::SocketAddr;

use http::StatusCode;
use monet::{Request, Router, get};

async fn hello(_req: Request) -> &'static str {
    "hello"
}

async fn global_notfound(_req: Request) -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "Page not Found")
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    let app1 = Router::new()
        .at("/hello", get(hello))
        .fallback(global_notfound);

    let app2 = Router::new()
        .at("/hi", get(hello))
        .fallback(global_notfound);

    // Should panic
    app1.merge(app2);
}
