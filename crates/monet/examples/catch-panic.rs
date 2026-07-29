use std::{net::SocketAddr, thread};

use monet::{CatchPanic, Request, Router, get};

async fn would_throw(_req: Request) -> String {
    panic!("catch me!")
}

fn main() {
    let core_ids = core_affinity::get_core_ids().expect("to succeed on *nix/win/macos platform");

    let handles = core_ids
        .into_iter()
        .map(|id| {
            thread::spawn(move || {
                println!("Thread Id: {:?}", id);
                _main();
            })
        })
        .collect::<Vec<_>>();

    for handle in handles.into_iter() {
        handle.join().unwrap();
    }
}

fn _main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    let app = Router::new()
        .at("/hello", get(would_throw))
        .wrap_by(CatchPanic);

    monet::run(addr, app);
}
