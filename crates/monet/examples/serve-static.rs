use std::net::SocketAddr;

use monet::{Router, ServeDir, service};

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    let app = Router::new().at("/hello.html", service(ServeDir::new("static")));

    monet::serve(addr, app);
}
