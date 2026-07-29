use std::net::SocketAddr;

use monet::{Error, Request, Router, get};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Pagination {
    pub page: i32,
    pub offset: i32,
}

async fn parse_query(req: Request) -> Result<String, Error> {
    let q = req.query::<Pagination>()?;
    Ok(q.offset.to_string())
}

// curl 0.0.0.0:9527?page=2&offset=5
// Returns 5
fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    let app = Router::new().at("/", get(parse_query));

    monet::run(addr, app);
}
