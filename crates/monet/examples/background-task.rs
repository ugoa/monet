use std::net::SocketAddr;

use monet::{Request, Router, get};

async fn spawn_background_task(_req: Request) -> &'static str {
    let bgtask = async {
        let second = 2;
        compio::runtime::time::sleep(std::time::Duration::from_millis(second * 1000)).await;
        println!(
            "Print to stdout after {} second, this will not block main handler",
            second
        );
    };
    monet::spawn(bgtask);
    "Immediately returned response"
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    let app = Router::new().at("/hello", get(spawn_background_task));

    monet::run(addr, app);
}
