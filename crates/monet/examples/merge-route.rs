use std::net::SocketAddr;

use monet::{Request, Router, get};

async fn hello(_req: Request) -> String {
    "Hello".to_string()
}

async fn hi(_req: Request) -> String {
    "Hi".to_string()
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    let app = Router::new().at("/hello", get(hello));
    let other = Router::new().at("/hi", get(hi));

    monet::run(addr, app.merge(other));
}
