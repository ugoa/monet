use std::net::SocketAddr;

use monet::{Request, Router, get};

async fn catch_all(req: Request) -> String {
    let matched = req.matched_path().unwrap();
    format!("nothing excapes by matcher: {matched}")
}

async fn hello(req: Request) -> String {
    let matched = req.matched_path().unwrap();
    format!("matched nested path: {matched}")
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    let nested = Router::new().at("/hello/{*rest}", get(hello));
    let app = Router::new()
        .at("/wild/{id}/card/{*another}", get(catch_all))
        .nest("/api", nested);

    monet::run(addr, app);
}
