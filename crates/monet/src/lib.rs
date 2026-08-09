pub mod body;
pub mod error;
pub mod handler;
pub mod listener;
pub mod request;
pub mod response;
pub mod router;
pub mod serve;
pub mod types;

// pub use monet_macros::handler;
pub use crate::{
    error::{BodyError, BoxError, Error},
    handler::{
        Endpoint, Middleware, endpoint::serve_dir::ServeDir, middleware::catch_panic::CatchPanic,
    },
    request::Request,
    response::{IntoResponse, Response},
    router::{Chain, Router, get, post},
    serve::{run, run_with_single_thread, spawn},
    types::{Form, Json, Path},
};

pub(crate) const NEVEL_FAIL: &str = "Should never fail. Please file a bug if it does";
