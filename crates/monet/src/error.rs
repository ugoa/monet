use std::{error::Error as StdError, fmt};

use http::StatusCode;
use thiserror::Error as ThisError;

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

#[derive(ThisError, Debug)]
pub enum LibError {
    #[error("Failed to deserialize the JSON body into the target type")]
    JsonDataError(#[from] serde_path_to_error::Error<serde_json::Error>),

    #[error("Failed to parse the request body as JSON")]
    JsonSyntaxError(#[from] serde_json::Error),

    #[error("Failed to buffer the request body")]
    UnknownBodyError(#[from] crate::Error),

    #[error("Json request must have `Content-Type: application/json`")]
    InvalidJsonContentType,

    #[error("Form request must have `Content-Type: application/x-www-form-urlencoded`")]
    InvalidFormContentType,

    #[error("Failed to deserialize form")]
    FailedToDeserializeForm(#[from] serde_path_to_error::Error<serde_html_form::de::Error>),
}

impl IntoResponse for LibError {
    fn into_response(self) -> Response {
        match self {
            Self::JsonDataError(e) => {
                let code = StatusCode::UNPROCESSABLE_ENTITY;
                (code, e.to_string()).into_response()
            }
            Self::JsonSyntaxError(e) => {
                let code = StatusCode::BAD_REQUEST;
                (code, e.to_string()).into_response()
            }
            Self::UnknownBodyError(e) => {
                let code = StatusCode::BAD_REQUEST;
                (code, e.to_string()).into_response()
            }
            Self::InvalidJsonContentType => {
                let code = StatusCode::UNSUPPORTED_MEDIA_TYPE;
                (code, self.to_string()).into_response()
            }
            Self::InvalidFormContentType => {
                let code = StatusCode::UNSUPPORTED_MEDIA_TYPE;
                (code, self.to_string()).into_response()
            }
            Self::FailedToDeserializeForm(_) => {
                let code = StatusCode::BAD_REQUEST;
                (code, self.to_string()).into_response()
            }
        }
    }
}
