use std::{error::Error as StdError, fmt};

use http::StatusCode;

use crate::response::{IntoResponse, Response};

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Errors that can happen when using axum.
#[derive(Debug)]
pub struct Error {
    inner: BoxError,
}

impl Error {
    /// Create a new `Error` from a boxable error.
    pub fn new(error: impl Into<BoxError>) -> Self {
        Self {
            inner: error.into(),
        }
    }

    /// Convert an `Error` back into the underlying boxed trait object.
    #[must_use]
    pub fn into_inner(self) -> BoxError {
        self.inner
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(&*self.inner)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum LibError {
    #[error("Failed to deserialize the JSON body into the target type")]
    JsonDataError,
}

impl IntoResponse for LibError {
    fn into_response(self) -> Response {
        match self {
            Self::JsonDataError => {
                let code = StatusCode::NETWORK_AUTHENTICATION_REQUIRED;
                (code, self.to_string()).into_response()
            }
        }
    }
}
