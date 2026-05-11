use std::net::SocketAddr;

use monet::{Router, ServeDir, get};

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    let app = Router::new().at("/assets/*files", get(ServeDir::new("static")));

    monet::serve(addr, app);
}
