use std::net::SocketAddr;

use http::header::HeaderValue;
use monet::{Request, Response, Router, get, handler};

#[handler]
async fn omni_api(resp: &mut Response) {
    resp.headers_mut()
        .insert("mark", HeaderValue::from_static("modified"));
}

async fn sample(req: Request) -> String {
    "Hi".to_string()
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Running http server from sub crate on {}", addr);

    let app = Router::new().at("/", get(sample));
    monet::serve(addr, app);
}
