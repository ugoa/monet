use crate::Body;
use crate::BoxError;
use crate::HttpResponse;
use std::borrow::Cow;
use std::convert::Infallible;

pub trait IntoResponse<'a> {
    /// Create a response.
    fn into_response(self) -> HttpResponse<'a>;
}

impl<'a, B> IntoResponse<'a> for HttpResponse<'a, B>
where
    B: http_body::Body<Data = bytes::Bytes> + 'static,
    B::Error: Into<BoxError>,
{
    fn into_response(self) -> HttpResponse<'a> {
        self.map(Body::new)
    }
}

impl<'a> IntoResponse<'a> for Body<'a> {
    fn into_response(self) -> HttpResponse<'a> {
        HttpResponse::new(self)
    }
}

impl<'a> IntoResponse<'a> for () {
    fn into_response(self) -> HttpResponse<'a> {
        Body::empty().into_response()
    }
}

impl<'a> IntoResponse<'a> for http::StatusCode {
    fn into_response(self) -> HttpResponse<'a> {
        let mut res = ().into_response();
        *res.status_mut() = self;
        res
    }
}

impl<'a> IntoResponse<'a> for Cow<'static, str> {
    fn into_response(self) -> HttpResponse<'a> {
        let res = Body::from(self).into_response();
        // res.headers_mut().insert(
        //     http::header::CONTENT_TYPE,
        //     http::HeaderValue::from_static(mime::TEXT_PLAIN_UTF_8.as_ref()),
        // );
        res
    }
}

impl<'a> IntoResponse<'a> for &'static str {
    fn into_response(self) -> HttpResponse<'a> {
        Cow::Borrowed(self).into_response()
    }
}

impl<'a> IntoResponse<'a> for String {
    fn into_response(self) -> HttpResponse<'a> {
        Cow::<'static, str>::Owned(self).into_response()
    }
}

impl<'a> IntoResponse<'a> for Box<str> {
    fn into_response(self) -> HttpResponse<'a> {
        String::from(self).into_response()
    }
}

impl<'a> IntoResponse<'a> for Infallible {
    fn into_response(self) -> HttpResponse<'a> {
        match self {}
    }
}
