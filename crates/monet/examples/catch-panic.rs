use std::net::SocketAddr;

use monet::{CatchPanic, Request, Router, get};

async fn throw(_req: Request) -> String {
    panic!("panic intentionlly")
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    let app = Router::new().at("/hello", get(throw)).wrap_by(CatchPanic);

    monet::run(addr, app);
}
