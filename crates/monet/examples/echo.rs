use std::net::SocketAddr;

use http::header::HeaderValue;
use monet::{Response, Router, serve::serve};

#[allow(non_camel_case_types)]
#[derive(Debug)]
struct index;
impl index {
    fn index(resp: &mut Response) {
        resp.headers_mut()
            .insert("mark", HeaderValue::from_static("modified"));
    }
}
impl monet::Handler for index {
    #[allow(
        elided_named_lifetimes,
        clippy::async_yields_async,
        clippy::diverging_sub_expression,
        clippy::let_unit_value,
        clippy::needless_arbitrary_self_type,
        clippy::no_effect_underscore_binding,
        clippy::shadow_same,
        clippy::type_complexity,
        clippy::type_repetition_in_bounds,
        clippy::used_underscore_binding
    )]
    fn handle<'life0, 'life1, 'life2, 'async_trait>(
        &'life0 self,
        req: &'life1 mut monet::Request,
        resp: &'life2 mut monet::Response,
    ) -> ::core::pin::Pin<Box<dyn ::core::future::Future<Output = ()> + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            let __self = self;
            let _: () = { Self::index(resp) };
        })
    }
}

async fn get_handler(resp: &mut Response) {
    resp.headers_mut()
        .insert("mark", HeaderValue::from_static("modified"));
}

async fn post_handler() {
    println!("should post")
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    println!("Running http server from sub crate on {}", addr);

    let mut app = Router::new();
    app.at("/").get(index);
    serve(addr, app);
}
