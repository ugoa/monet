use std::net::SocketAddr;

use monet::Router;

/// To Succeed, please run this one from monet project root.
/// cargo run --examples serve-static
fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    monet::run(addr, || {
        Router::new().serve_dir("/assets", "./crates/monet/examples/assets")
    });
}
