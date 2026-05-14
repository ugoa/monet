use std::net::SocketAddr;

use monet::{Error, Json, Request, Router, get};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Deserialize, Serialize)]
pub struct UserPayload {
    pub email: String,
    pub password: String,
}

async fn parse_json(req: Request) -> Result<Json<UserPayload>, Error> {
    req.into_json().await
}

// Return a serde_json::Value using the json! macro
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

    let app = Router::new().at("/", get(return_json).post(parse_json));

    monet::run(addr, app);
}
