use std::{net::SocketAddr, thread};

use monet::{Request, Router, get};

async fn greeting(_req: Request) -> String {
    "Hello from monet with single thread mode".to_string()
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    let app = Router::new().at("/", get(greeting)).at(
        "/hi",
        get(async |_req: Request| format!("closure works too")),
    );

    monet::run_with_single_thread(addr, app);
}
