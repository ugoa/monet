use bytes::Bytes;
use http::{Response as HttpResponse, StatusCode};
use http_body_util::Full;

pub type Response = HttpResponse<Full<Bytes>>;

pub trait IntoResponse {
    #[must_use]
    fn into_response(self) -> Response;
}

impl IntoResponse for String {
    fn into_response(self) -> Response {
        Response::new(Full::new(Bytes::from(self)))
    }
}

impl IntoResponse for &'static str {
    fn into_response(self) -> Response {
        Response::new(Full::new(Bytes::from(self)))
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
