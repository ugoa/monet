use std::{collections::HashMap, net::SocketAddr, sync::Mutex, thread};

use monet::{Request, Response, Router, State, get, response::*};
use tracing::info;

async fn greeting(_req: Request, State(ctx): State<&MyContext>) -> Response {
    format!("Current thread ID: {:?}", thread::current().id()).into_response()
}

#[derive(Default)]
struct MyContext {
    s: String,
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    info!("Server running at: {}", addr);

    let ctx = MyContext {
        s: "Global".to_string(),
    };

    monet::Server::new(addr, || {
        Router::new().at("/", get(greeting)).at(
            "/hi",
            get(async |_req: Request| format!("closure works too")),
        )
    })
    .run(&ctx);
}
