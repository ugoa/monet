use std::net::SocketAddr;

use http::header::HeaderValue;
use monet::{Response, Router, get, handler};

#[handler]
async fn get_handler(resp: &mut Response) {
    resp.headers_mut()
        .insert("mark", HeaderValue::from_static("modified"));
}

#[handler]
async fn post_handler() {
    println!("should post")
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Running http server from sub crate on {}", addr);

    let app = Router::new().at("/", get(get_handler).post(post_handler));
    monet::serve(addr, app);
}
