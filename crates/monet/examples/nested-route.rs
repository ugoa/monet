use std::net::SocketAddr;

use monet::{Request, Router, get};

async fn hello(_req: Request) -> String {
    "Hello from /public/hello".to_string()
}

async fn hi(_req: Request) -> String {
    "Hello from /public/hi".to_string()
}

async fn private(_req: Request) -> String {
    "Hello from /private".to_string()
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    let public = Router::new().at("/hello", get(hello)).at("/hi", get(hi));

    let private = Router::new().at("/secret/{*rest}", get(private));

    let app = Router::new()
        .nest("/public", public)
        .nest("/private", private);

    monet::run(addr, app);
}
