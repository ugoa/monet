use std::net::SocketAddr;

use monet::{Request, Router, get};

async fn hello(_req: Request) -> String {
    "Hello from /api/hello".to_string()
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    let nested = Router::new().at("/hello", get(hello));

    let app = Router::new().nest("/api", nested);

    monet::serve(addr, app);
}
