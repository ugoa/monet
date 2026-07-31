use std::net::SocketAddr;

use monet::{Router, get, request::Request, types::Html};

async fn return_html(_req: Request) -> Html<&'static str> {
    Html(
        r#"
        <!doctype html>
        <html>
            <head>
                <title>Hello from Monet </title>
            </head>
            <body>
                <h3>Welcome!</h3>
            </body>
        </html>
        "#,
    )
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Server running at: {}", addr);

    monet::run(addr, || Router::new().at("/", get(return_html)));
}
