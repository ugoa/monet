use std::{net::SocketAddr, time::Duration};

use compio::runtime::time::sleep;
use monet::{Request, Router, get};

async fn spawn_background_task(_req: Request) -> &'static str {
    monet::spawn(async {
        let sec = 2;
        sleep(Duration::from_millis(sec * 1000)).await;
        println!("Print to stdout after {sec} second, this will not block main handler",);
    });
    "Immediately returned response"
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    monet::Server::new(addr, || Router::new().at("/", get(spawn_background_task))).run();
}
