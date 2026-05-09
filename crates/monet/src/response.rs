use bytes::{BufMut, Bytes, BytesMut};
use http::{HeaderValue, Response as HttpResponse, StatusCode, header};
use http_body_util::Full;
use serde::Serialize;

use crate::{body::Body, json::Json};

pub type Response<T = Body> = HttpResponse<T>;

pub trait IntoResponse {
    #[must_use]
    fn into_response(self) -> Response;
}

impl IntoResponse for Body {
    fn into_response(self) -> Response {
        Response::new(self)
    }
}

impl IntoResponse for String {
    fn into_response(self) -> Response {
        Body::new(http_body_util::Full::from(self)).into_response()
    }
}

impl IntoResponse for &'static str {
    fn into_response(self) -> Response {
        Body::new(http_body_util::Full::from(self)).into_response()
    }
}

impl<R> IntoResponse for (StatusCode, R)
where
    R: IntoResponse,
{
    fn into_response(self) -> Response {
        let mut res = self.1.into_response();
        *res.status_mut() = self.0;
        res
    }
}

impl IntoResponse for StatusCode {
    fn into_response(self) -> Response {
        let mut res = ().into_response();
        *res.status_mut() = self;
        res
    }
}

impl IntoResponse for () {
    fn into_response(self) -> Response {
        Response::new(Body::empty())
    }
}

impl IntoResponse for Bytes {
    fn into_response(self) -> Response {
        let mut res = Body::from(self).into_response();
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(mime::APPLICATION_OCTET_STREAM.as_ref()),
        );
        res
    }
}

impl<T> IntoResponse for Json<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        // Extracted into separate fn so it's only compiled once for all T.
        fn make_response(buf: BytesMut, ser_result: serde_json::Result<()>) -> Response {
            match ser_result {
                Ok(()) => (
                    [(
                        header::CONTENT_TYPE,
                        HeaderValue::from_static(mime::APPLICATION_JSON.as_ref()),
                    )],
                    buf.freeze(),
                )
                    .into_response(),
                Err(err) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(
                        header::CONTENT_TYPE,
                        HeaderValue::from_static(mime::TEXT_PLAIN_UTF_8.as_ref()),
                    )],
                    err.to_string(),
                )
                    .into_response(),
            }
        }

        // Use a small initial capacity of 128 bytes like serde_json::to_vec
        // https://docs.rs/serde_json/1.0.82/src/serde_json/ser.rs.html#2189
        let mut buf = BytesMut::with_capacity(128).writer();
        let res = serde_json::to_writer(&mut buf, &self.0);
        make_response(buf.into_inner(), res)
    }
}
