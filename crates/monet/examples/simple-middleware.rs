use std::net::SocketAddr;

use monet::{Chain, Request, Response, Router, get};

async fn endpoint(_req: Request) -> &'static str {
    println!("endpoint");
    "endpoint"
}

async fn inner(req: Request, chain: Chain) -> Response {
    println!("inner");
    chain.next(req).await
}

async fn outer(req: Request, chain: Chain) -> Response {
    println!("outer");
    chain.next(req).await
}

async fn outmost(req: Request, chain: Chain) -> Response {
    println!("outmost");
    chain.next(req).await
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    monet::run(addr, || {
        Router::new()
            .at("/", get(endpoint))
            .wrap_by(inner)
            .wrap_by(outer)
            .wrap_by(outmost)
    });
}
