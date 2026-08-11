use std::{net::SocketAddr, thread};

use monet::{Request, Router, get};
use tracing::info;

async fn greeting(_req: Request) -> String {
    format!(
        "{} Current thread ID: {:?}",
        jiff::Zoned::now(),
        thread::current().id()
    )
}

fn main() {
    tracing_subscriber::fmt::init();

    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    info!("Server running at: {}", addr);

    monet::run(addr, || {
        Router::new().at("/", get(greeting)).at(
            "/hi",
            get(async |_req: Request| format!("closure works too")),
        )
    });
}
