use std::{panic::AssertUnwindSafe, pin::Pin};

use futures_util::FutureExt;
use http::StatusCode;

use crate::{IntoResponse, Layer, Middleware, Request, Response};

#[derive(Default, Debug, Clone)]
pub struct CatchPanic;

impl Middleware for CatchPanic {
    fn transform(
        &self,
        req: Request,
        layer: Layer,
    ) -> Pin<Box<dyn Future<Output = Response> + '_>> {
        Box::pin(async move {
            AssertUnwindSafe(layer.next(req))
                .catch_unwind()
                .await
                .unwrap_or_else(|err| {
                    tracing::error!(error = ?err, "panic occurred");

                    let mut resp = "Service panicked".into_response();
                    *resp.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                    resp
                })
        })
    }
}
