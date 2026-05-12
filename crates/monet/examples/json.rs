use std::net::SocketAddr;

use monet::{Json, Request, Router, get};
use serde_json::{Value, json};

// Create a serde_json::Value using the json! macro
async fn return_json(_req: Request) -> Json<Value> {
    let data = json!({
        "status": "success",
        "data": { "id": 1, "name": "example" }
    });

    Json(data)
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    let app = Router::new().at("/", get(return_json));

    monet::serve(addr, app);
}
