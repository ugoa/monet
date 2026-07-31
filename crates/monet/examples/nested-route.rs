use std::net::SocketAddr;

use monet::{Request, Router, get};

async fn hello(_req: Request) -> String {
    "Hello from /public/hello".to_string()
}

async fn hi(_req: Request) -> String {
    "Hello from /public/hi".to_string()
}

async fn secret_reveal(_req: Request) -> String {
    "Hello from /private/secrets/12".to_string()
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    monet::run(addr, || {
        Router::new()
            .nest(
                "/public",
                Router::new().at("/hello", get(hello)).at("/hi", get(hi)),
            )
            .nest(
                "/private",
                Router::new().at("/secret/{*rest}", get(secret_reveal)),
            )
    });
}
