use std::net::SocketAddr;

use monet::serve::serve;

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Running http server on {}", addr);
    serve(addr);
}
