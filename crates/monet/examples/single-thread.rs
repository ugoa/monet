use std::net::SocketAddr;

use monet::{Request, Router, get};

async fn light(_req: Request) -> String {
    let received = jiff::Zoned::now();
    compio::runtime::time::sleep(std::time::Duration::from_millis(2000)).await;
    let handled = jiff::Zoned::now();

    format!("Request received at {received}, response sent at {handled}\n",)
}

async fn heavy(_req: Request) -> String {
    let received = jiff::Zoned::now();
    compio::runtime::time::sleep(std::time::Duration::from_millis(4000)).await;
    let handled = jiff::Zoned::now();

    format!("Request received at {received}, response sent at {handled}\n",)
}

// Test in nushell:
//  cat urls.txt | lines | par-each { |url| curl -s $url }
// In urls.txt:
//  0.0.0.0:9527/light
//  0.0.0.0:9527/heavy
fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    let app = Router::new()
        .at("/light", get(light))
        .at("/heavy", get(heavy));

    monet::run_with_single_thread(addr, app);
}
