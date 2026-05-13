use std::net::SocketAddr;

use monet::{Request, Router, get, post};

async fn hello(_req: Request) -> &'static str {
    "hello"
}

async fn hi(_req: Request) -> &'static str {
    "hello"
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    let app5 = Router::new().at("/hi", get(hi));
    let app6 = Router::new().at("/hello", post(hello));

    println!("Server running at: {}", addr);
    monet::run(addr, app5.merge(app6));
}
