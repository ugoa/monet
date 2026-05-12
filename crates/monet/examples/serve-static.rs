use std::net::SocketAddr;

use monet::Router;

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    let app = Router::new().serve_dir("/assets", "./crates/monet/examples/assets");

    monet::run(addr, app);
}
