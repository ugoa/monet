use std::{
    io,
    net::{self, SocketAddr},
};

use monet::{Request, Router, get};

async fn greeting(_req: Request) -> &'static str {
    "Hi, from monet"
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    let app = Router::new().at("/", get(greeting));

    monet::run2(addr);
}
