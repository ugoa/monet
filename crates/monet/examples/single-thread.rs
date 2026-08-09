use std::net::SocketAddr;

use monet::{Request, Router, get};

async fn greeting(_req: Request) -> String {
    let received = jiff::Zoned::now();
    // compio::runtime::time::sleep(std::time::Duration::from_millis(2000)).await;
    let handled = jiff::Zoned::now();

    format!("Request received at {received}, response sent at {handled}",)
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
