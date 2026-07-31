use std::net::SocketAddr;

use monet::{Request, Router, get, post};

async fn hello(_req: Request) -> &'static str {
    "hello"
}

async fn hi(_req: Request) -> &'static str {
    "hi"
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    monet::run(addr, || {
        let app5 = Router::new().at("/hi", get(hi));
        let app6 = Router::new().at("/hello", post(hello));
        app5.merge(app6)
    });
}
