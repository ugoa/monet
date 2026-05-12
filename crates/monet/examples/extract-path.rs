use std::net::SocketAddr;

use monet::{Request, Router, extract::path::Path, get};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Expected {
    id: i32,
    name: String,
}

async fn parse_path(req: Request) -> String {
    let path: Path<Expected> = req.path().expect("Wildcard params should be parsed fine");
    format!("Received id: {}, name: {}", path.id, path.name)
}

// curl http://0.0.0.0:9527/wild/8797/card/larry.
// Expect: Received id: 8797, name: larry
fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    let app = Router::new().at("/wild/{id}/card/{*name}", get(parse_path));

    monet::run(addr, app);
}
