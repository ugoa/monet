use std::{error::Error as StdError, fmt};

use http::StatusCode;
use thiserror::Error as ThisError;

use crate::response::{IntoResponse, Response};

#[derive(ThisError, Debug)]
pub enum Error {
    #[error("Failed to deserialize the JSON body into the target type: {0}")]
    JsonDataError(#[from] serde_path_to_error::Error<serde_json::Error>),

    #[error("Failed to parse the request body as JSON: {0}")]
    JsonSyntaxError(#[from] serde_json::Error),

    #[error("Failed to buffer the request body: {0}")]
    UnknownBodyError(#[from] crate::BodyError),

    #[error("Json request must have `Content-Type: application/json`")]
    InvalidJsonContentType,

    #[error("Form request must have `Content-Type: application/x-www-form-urlencoded`")]
    InvalidFormContentType,

    #[error("Failed to deserialize Form: {0}")]
    FailedToDeserializeForm(#[source] serde_path_to_error::Error<serde_html_form::de::Error>),

    #[error("Failed to deserialize Query: {0}")]
    FailedToDeserializeQuery(#[source] serde_path_to_error::Error<serde_urlencoded::de::Error>),

    #[error("No paths parameters found for matched route")]
    MissingPathParams,

    #[error("Invalid UTF-8 in path parameters found for matched route")]
    InvalidUtf8InPathParam { key: String },

    #[error("Failed to Deserialize Path params for many reasons")]
    FailedToDeserializePathParams,
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status_code = match self {
            Self::JsonDataError(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::JsonSyntaxError(_) => StatusCode::BAD_REQUEST,
            Self::UnknownBodyError(_) => StatusCode::BAD_REQUEST,
            Self::InvalidJsonContentType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Self::InvalidFormContentType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Self::FailedToDeserializeForm(_) => StatusCode::BAD_REQUEST,
            Self::FailedToDeserializeQuery(_) => StatusCode::BAD_REQUEST,
            Self::MissingPathParams => StatusCode::INTERNAL_SERVER_ERROR,
            Self::InvalidUtf8InPathParam { key: _ } => StatusCode::INTERNAL_SERVER_ERROR,
            Self::FailedToDeserializePathParams => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status_code, self.to_string()).into_response()
    }
}

#[derive(Debug)]
pub struct BodyError {
    inner: BoxError,
}

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

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
