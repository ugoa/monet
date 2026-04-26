use std::net::SocketAddr;

use monet::{Router, serve::serve};

async fn get_handler() {
    println!("should get")
}

async fn post_handler() {
    println!("should post")
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Running http server on {}", addr);

    let mut app = Router::new();
    app.at("/").get(get_handler).post(post_handler);
    serve(addr, app);
}
