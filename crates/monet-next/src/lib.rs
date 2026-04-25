// pub mod handler;
pub mod serve;

use bytes::Bytes;
use std::{net::ToSocketAddrs, pin::Pin};

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

pub struct Body(Pin<Box<dyn http_body::Body<Data = Bytes, Error = BoxError>>>);

pub type Request<T = Body> = http::Request<T>;
pub type Response<T = Body> = http::Response<T>;
