// pub mod handler;
pub mod serve;

use bytes::Bytes;
use std::pin::Pin;

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

pub struct Body(Pin<Box<dyn http_body::Body<Data = Bytes, Error = BoxError>>>);

pub type Request = http::Request<Body>;
pub type Response = http::Response<Body>;

pub(crate) trait Handler {
    async fn call(&self, req: &mut Request, resp: &mut Response);
}
