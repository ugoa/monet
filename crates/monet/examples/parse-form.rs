use std::net::SocketAddr;

use monet::{Error, Form, Request, Router, post};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct FormPayload {
    pub name: String,
    pub email: String,
}
async fn parse_form(req: Request) -> Result<Form<FormPayload>, Error> {
    req.into_form().await
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    let app = Router::new().at("/", post(parse_form));

    monet::run(addr, app);
}
