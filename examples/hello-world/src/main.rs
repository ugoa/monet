use monet::serve::serve;
use std::net::SocketAddr;

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Running http server on {}", addr);
    serve(addr);
}
