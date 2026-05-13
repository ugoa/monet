use std::net::SocketAddr;

use http::StatusCode;
use monet::{Request, Router, get, post};

async fn hello(_req: Request) -> &'static str {
    "hello"
}

async fn hi(_req: Request) -> &'static str {
    "hello"
}

async fn notfound(_req: Request) -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "Page not Found")
}

async fn no_support(_req: Request) -> String {
    format!("No support for {} METHOD at this route", _req.method())
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();

    let app1 = Router::new().at("/hello", get(hello)).catch_all(notfound);
    let app2 = Router::new().at("/hi", get(hello)).catch_all(notfound);
    // Should panic
    // app1.merge(app2);

    let app3 = Router::new().at("/hello", get(hello).catch(no_support));
    let app4 = Router::new().at("/hi", post(hello).catch(no_support));
    // Should panic
    // app3.merge(app4);

    let app5 = Router::new().at("/hi", get(hello));
    let app6 = Router::new().at("/hi", post(hello));
    // Should Succeed
    // app5.merge(app6);

    let app5 = Router::new().at("/hi", get(hello));
    let app6 = Router::new().at("/hi", get(hi));
    // Should panic
    app5.merge(app6);
}
