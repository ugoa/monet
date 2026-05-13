use std::{any::Any, panic::AssertUnwindSafe};

use async_trait::async_trait;
use futures_util::FutureExt;
use http::{HeaderValue, StatusCode};

use crate::{Chain, IntoResponse, Middleware, Request, Response};

#[derive(Default, Debug)]
pub struct CatchPanic;

#[async_trait(?Send)]
impl Middleware for CatchPanic {
    async fn transform(&self, req: Request, chain: Chain) -> Response {
        AssertUnwindSafe(chain.next(req))
            .catch_unwind()
            .await
            .unwrap_or_else(|err| {
                tracing::error!(error = ?err, "panic occurred");

                let mut resp = "Service panicked".into_response();
                *resp.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;

                let text = HeaderValue::from_static("text/plain; charset=utf-8");
                resp.headers_mut().insert(http::header::CONTENT_TYPE, text);

                resp
            })
    }
}
