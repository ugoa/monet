use std::{
    cell::{LazyCell, RefCell},
    net::SocketAddr,
    pin::Pin,
    sync::{Arc, LazyLock, Mutex},
};

use http::header::HeaderValue;
use monet::{Chain, Middleware, Response, Router, get, request::Request, types::Html};

async fn metric(req: Request) -> String {
    format!(
        "current count: {}",
        req.state.get::<CurrentCount>().expect("success").0
    )
}

#[derive(Clone)]
struct RequestCount {
    local_count: RefCell<usize>,
}

#[derive(Clone)]
struct CurrentCount(usize);

impl Middleware for RequestCount {
    fn transform(
        &self,
        mut req: Request,
        chain: Chain,
    ) -> Pin<Box<dyn Future<Output = Response> + '_>> {
        let mut s = self.local_count.borrow_mut();
        *s += 1;
        drop(s);
        req.state.set(CurrentCount(*self.local_count.borrow()));

        Box::pin(async move { chain.next(req).await })
    }
}

fn main() {
    let addr = "0.0.0.0:9527";
    println!("Server running at: {}", addr);

    monet::run_with_single_thread(
        addr,
        Router::new().at("/", get(metric)).wrap_by(RequestCount {
            local_count: RefCell::new(0),
        }),
    );
}
