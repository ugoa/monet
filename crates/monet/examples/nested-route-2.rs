use std::net::SocketAddr;

use monet::{Path, Request, Router, get};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Scan {
    user_id: i32,
    post_id: i32,
}

// curl 0.0.0.0:9527/users/35/posts/7
// expect: User 35 posted #7
async fn analysis(req: Request) -> String {
    let path: Path<Scan> = req.path().unwrap();
    format!("User {} posted #{}", path.user_id, path.post_id)
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    monet::run(addr, || {
        Router::new().nest(
            "/users/{user_id}",
            Router::new().at("/posts/{post_id}", get(analysis)),
        )
    });
}
