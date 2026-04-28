use std::net::SocketAddr;

use http::header::HeaderValue;
use monet::{Response, Router, get, handler};

#[handler]
async fn omni_api(resp: &mut Response) {
    resp.headers_mut()
        .insert("mark", HeaderValue::from_static("modified"));
}

#[handler]
async fn sample() {
    println!("should post")
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Running http server from sub crate on {}", addr);

    let app = Router::new().at(
        "/",
        get(omni_api)
            .post(omni_api)
            .connect(omni_api)
            .delete(omni_api)
            .patch(omni_api)
            .put(omni_api)
            .options(omni_api)
            .head(omni_api)
            .trace(omni_api),
    );
    monet::serve(addr, app);
}
