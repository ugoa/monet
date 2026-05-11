use std::net::SocketAddr;

use monet::{Router, get, handler::endpoint::serve_dir::ServeDir};

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    let app = Router::new().at("/hello.html", get(ServeDir::new("static")));

    monet::serve(addr, app);
}
