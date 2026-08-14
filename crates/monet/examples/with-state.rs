use std::sync::Arc;

use futures::lock::Mutex;
use monet::{Request, Router, get};

type GlobalCounter = Arc<Mutex<usize>>;

async fn greeting(req: Request) -> String {
    let mut cnt = req.extensions.get::<GlobalCounter>().unwrap().lock().await;
    *cnt += 1;
    format!("Request count is: {:?}", *cnt)
}

fn main() {
    let addr = "0.0.0.0:9527";
    tracing::info!("Server running at: {}", addr);

    let s: GlobalCounter = Arc::new(Mutex::new(0));
    monet::Server::new(addr, move || {
        Router::new().at("/", get(greeting)).with_state(s.clone())
    })
    .run()
}
