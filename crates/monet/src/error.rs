use std::{error::Error as StdError, fmt};

use http::StatusCode as Code;
use thiserror::Error as ThisError;

use crate::response::{IntoResponse, Response};

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Errors that can happen when using axum.
#[derive(Debug)]
pub struct BodyError {
    inner: BoxError,
}

impl BodyError {
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

impl fmt::Display for BodyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl StdError for BodyError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(&*self.inner)
    }
}

#[derive(ThisError, Debug)]
pub enum Error {
    #[error("Failed to deserialize the JSON body into the target type")]
    JsonDataError(#[from] serde_path_to_error::Error<serde_json::Error>),

    #[error("Failed to parse the request body as JSON")]
    JsonSyntaxError(#[from] serde_json::Error),

    #[error("Failed to buffer the request body")]
    UnknownBodyError(#[from] crate::BodyError),

    #[error("Json request must have `Content-Type: application/json`")]
    InvalidJsonContentType,

    #[error("Form request must have `Content-Type: application/x-www-form-urlencoded`")]
    InvalidFormContentType,

    #[error("Failed to deserialize form")]
    FailedToDeserializeForm(#[source] serde_path_to_error::Error<serde_html_form::de::Error>),

    #[error("Failed to deserialize query")]
    FailedToDeserializeQuery(#[source] serde_path_to_error::Error<serde_urlencoded::de::Error>),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        match self {
            Self::JsonDataError(e) => (Code::UNPROCESSABLE_ENTITY, e.to_string()).into_response(),
            Self::JsonSyntaxError(e) => (Code::BAD_REQUEST, e.to_string()).into_response(),
            Self::UnknownBodyError(e) => (Code::BAD_REQUEST, e.to_string()).into_response(),
            Self::InvalidJsonContentType => {
                (Code::UNSUPPORTED_MEDIA_TYPE, self.to_string()).into_response()
            }
            Self::InvalidFormContentType => {
                (Code::UNSUPPORTED_MEDIA_TYPE, self.to_string()).into_response()
            }
            Self::FailedToDeserializeForm(_) => {
                (Code::BAD_REQUEST, self.to_string()).into_response()
            }
            Self::FailedToDeserializeQuery(_) => {
                (Code::BAD_REQUEST, self.to_string()).into_response()
            }
        }
    }
}
