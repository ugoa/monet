use std::{net::SocketAddr, thread};

use monet::{Request, Router, get};

async fn greeting(_req: Request) -> String {
    format!("Current thread ID: {:?}", thread::current().id())
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    monet::run(addr, || {
        Router::new().at("/", get(greeting)).at(
            "/hi",
            get(async |_req: Request| format!("closure works too")),
        )
    });
}
