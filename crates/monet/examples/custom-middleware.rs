use std::{cell::RefCell, pin::Pin};

use monet::{Chain, Middleware, Request, Response, Router, get};

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
        req.extensions
            .insert(CurrentCount(*self.local_count.borrow()));

        Box::pin(async move { chain.next(req).await })
    }
}

async fn metric(req: Request) -> String {
    format!(
        "current count: {}",
        req.extensions.get::<CurrentCount>().expect("success").0
    )
}

fn main() {
    let addr = "0.0.0.0:9527";
    println!("Server running at: {}", addr);

    monet::SingleThreadServer::new(
        addr,
        Router::new().at("/", get(metric)).wrap_by(RequestCount {
            local_count: RefCell::new(0),
        }),
    )
    .run();
}
