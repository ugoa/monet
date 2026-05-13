use http::StatusCode;

use crate::{Request, Router, get, post};

#[test]
#[should_panic(expected = "Overlapping route. Cannot add two endpoints that both handle `GET`")]
fn merge_test1() {
    let app5 = Router::new().at("/hi", get(hello));
    let app6 = Router::new().at("/hi", get(hi));
    app5.merge(app6);
}

#[test]
#[should_panic(expected = "Cannot merge two `Route`s of same path that both have a fallback")]
fn merge_test2() {
    let app3 = Router::new().at("/hello", get(hello).catch(no_support));
    let app4 = Router::new().at("/hello", post(hi).catch(no_support));
    app3.merge(app4);
}

#[test]
#[should_panic(expected = "Cannot merge two `Router`s that both have a fallback")]
fn merge_test3() {
    let app1 = Router::new().at("/hello", get(hello)).catch_all(notfound);
    let app2 = Router::new().at("/hi", get(hi)).catch_all(notfound);
    app1.merge(app2);
}

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
