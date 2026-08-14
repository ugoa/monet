use std::net::SocketAddr;

use monet::{CatchPanic, Request, Router, get};

async fn would_throw(_req: Request) -> String {
    panic!("catch me! then continue to accept requests!")
}

async fn normal(_req: Request) -> String {
    "Normal as usual".to_string()
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    monet::Server::new(addr, || {
        Router::new()
            .at("/", get(would_throw))
            .wrap_by(CatchPanic)
            .at("/normal", get(normal))
    })
    .run();
}
