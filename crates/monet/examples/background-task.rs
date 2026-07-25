use std::net::SocketAddr;

use monet::{Request, Router, get};

async fn spawn_background_task(_req: Request) -> &'static str {
    let bg_task = async {
        let sec = 2;
        compio::runtime::time::sleep(std::time::Duration::from_millis(sec * 1000)).await;
        println!("Print to stdout after {sec} second, this will not block main handler",);
    };
    monet::spawn(bg_task);
    "Immediately returned response"
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    let app = Router::new().at("/hello", get(spawn_background_task));

    monet::run(addr, app);
}
