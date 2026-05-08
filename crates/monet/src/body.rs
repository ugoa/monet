use bytes::Bytes;

use crate::error::Error;

#[must_use]
#[derive(Debug)]
pub struct Body(BoxBody);

type BoxBody = http_body_util::combinators::UnsyncBoxBody<Bytes, Error>;
