use std::{net::SocketAddr, thread};

use monet::{CatchPanic, Request, Router, get};

async fn would_throw(_req: Request) -> String {
    panic!("catch me! then continue to accept requests!")
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    monet::run(addr, || {
        Router::new().at("/", get(would_throw)).wrap_by(CatchPanic)
    });
}
