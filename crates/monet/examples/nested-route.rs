use std::net::SocketAddr;

use monet::{Request, Router, get};

async fn hello(_req: Request) -> String {
    "Hello from /api/hello".to_string()
}

async fn hi(_req: Request) -> String {
    "Hello from /api/hi".to_string()
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    let nested = Router::new().at("/hello", get(hello)).at("/hi", get(hi));

    let app = Router::new().nest("/api", nested);

    monet::serve(addr, app);
}
