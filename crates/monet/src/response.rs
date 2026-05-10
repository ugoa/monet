use std::borrow::Cow;

use bytes::{BufMut, Bytes, BytesMut};
use http::{HeaderMap, HeaderValue, Response as HttpResponse, StatusCode, header::CONTENT_TYPE};
use serde::Serialize;

use crate::{BoxError, Form, Json, body::Body, types::Html};

pub type Response<T = Body> = HttpResponse<T>;

pub trait IntoResponse {
    #[must_use]
    fn into_response(self) -> Response;
}

impl<B> IntoResponse for Response<B>
where
    B: http_body::Body<Data = Bytes> + Send + 'static,
    B::Error: Into<BoxError>,
{
    fn into_response(self) -> Response {
        self.map(Body::new)
    }
}

impl IntoResponse for Body {
    fn into_response(self) -> Response {
        Response::new(self)
    }
}

impl IntoResponse for String {
    fn into_response(self) -> Response {
        Response::new(Body::new(http_body_util::Full::from(self)))
    }
}

impl IntoResponse for &'static str {
    fn into_response(self) -> Response {
        Response::new(Body::new(http_body_util::Full::from(self)))
    }
}

impl IntoResponse for Box<str> {
    fn into_response(self) -> Response {
        Response::new(Body::new(http_body_util::Full::from(String::from(self))))
    }
}

impl IntoResponse for Cow<'static, str> {
    fn into_response(self) -> Response {
        let mut res = Response::new(Body::new(http_body_util::Full::from(self)));
        res.headers_mut().insert(
            CONTENT_TYPE,
            HeaderValue::from_static(mime::TEXT_PLAIN_UTF_8.as_ref()),
        );
        res
    }
}

impl IntoResponse for (StatusCode, String) {
    fn into_response(self) -> Response {
        let mut resp = Response::new(Body::new(http_body_util::Full::from(self.1)));
        *resp.status_mut() = self.0;
        resp
    }
}

impl IntoResponse for (StatusCode, &'static str) {
    fn into_response(self) -> Response {
        let mut resp = Response::new(Body::new(http_body_util::Full::from(self.1)));
        *resp.status_mut() = self.0;
        resp
    }
}

impl IntoResponse for StatusCode {
    fn into_response(self) -> Response {
        let mut res = Response::new(Body::empty());
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
        let mut res = Response::new(Body::new(http_body_util::Full::from(self)));
        res.headers_mut().insert(
            CONTENT_TYPE,
            HeaderValue::from_static(mime::APPLICATION_OCTET_STREAM.as_ref()),
        );
        res
    }
}

impl IntoResponse for BytesMut {
    fn into_response(self) -> Response {
        self.freeze().into_response()
    }
}

impl IntoResponse for Cow<'static, [u8]> {
    fn into_response(self) -> Response {
        let mut res = Response::new(Body::new(http_body_util::Full::from(self)));
        res.headers_mut().insert(
            CONTENT_TYPE,
            HeaderValue::from_static(mime::APPLICATION_OCTET_STREAM.as_ref()),
        );
        res
    }
}

impl IntoResponse for Vec<u8> {
    fn into_response(self) -> Response {
        Cow::<'static, [u8]>::Owned(self).into_response()
    }
}

impl IntoResponse for HeaderMap {
    fn into_response(self) -> Response {
        let mut res = Response::new(Body::empty());
        *res.headers_mut() = self;
        res
    }
}

impl<T, E> IntoResponse for Result<T, E>
where
    T: IntoResponse,
    E: IntoResponse,
{
    fn into_response(self) -> Response {
        match self {
            Ok(value) => value.into_response(),
            Err(err) => err.into_response(),
        }
    }
}

impl<T> IntoResponse for Json<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        // Use a small initial capacity of 128 bytes like serde_json::to_vec
        // https://docs.rs/serde_json/1.0.82/src/serde_json/ser.rs.html#2189
        let mut buf = BytesMut::with_capacity(128).writer();
        let ser_result = serde_json::to_writer(&mut buf, &self.0);
        let buf = buf.into_inner();

        match ser_result {
            Ok(()) => {
                let mut resp = buf.freeze().into_response();
                resp.headers_mut().insert(
                    CONTENT_TYPE,
                    HeaderValue::from_static(mime::APPLICATION_JSON.as_ref()),
                );
                resp
            }
            Err(err) => {
                let mut resp = err.to_string().into_response();
                resp.headers_mut().insert(
                    CONTENT_TYPE,
                    HeaderValue::from_static(mime::TEXT_PLAIN_UTF_8.as_ref()),
                );
                *resp.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                resp
            }
        }
    }
}

impl<T> IntoResponse for Form<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        match serde_urlencoded::to_string(&self.0) {
            Ok(body) => {
                let mut resp = body.into_response();
                resp.headers_mut().insert(
                    CONTENT_TYPE,
                    HeaderValue::from_static(mime::APPLICATION_WWW_FORM_URLENCODED.as_ref()),
                );
                resp
            }
            Err(err) => {
                let mut resp = err.to_string().into_response();
                resp.headers_mut().insert(
                    CONTENT_TYPE,
                    HeaderValue::from_static(mime::TEXT_PLAIN_UTF_8.as_ref()),
                );
                *resp.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                resp
            }
        }
    }
}

impl<T> IntoResponse for Html<T>
where
    T: IntoResponse,
{
    fn into_response(self) -> Response {
        let mut resp = self.0.into_response();
        resp.headers_mut().insert(
            CONTENT_TYPE,
            HeaderValue::from_static(mime::TEXT_HTML_UTF_8.as_ref()),
        );
        resp
    }
}
