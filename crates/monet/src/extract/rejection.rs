use crate::{
    __composite_rejection as composite_rejection, __define_rejection as define_rejection, BoxError,
    Error,
};

// define_rejection! {
//     #[status = UNPROCESSABLE_ENTITY]
//     #[body = "Failed to deserialize the JSON body into the target type"]
//     /// Rejection type for [`Json`](super::Json).
//     ///
//     /// This rejection is used if the request body is syntactically valid JSON but couldn't be
//     /// deserialized into the target type.
//     pub struct JsonDataError(Error);
// }

#[doc = r" Rejection type for [`Json`](super::Json)."]
#[doc = r""]
#[doc = r" This rejection is used if the request body is syntactically valid JSON but couldn't be"]
#[doc = r" deserialized into the target type."]
#[derive(Debug)]
pub struct JsonDataError(pub(crate) crate::Error);
impl JsonDataError {
    pub(crate) fn from_err<E>(err: E) -> Self
    where
        E: Into<crate::BoxError>,
    {
        Self(crate::Error::new(err))
    }
    #[doc = r" Get the response body text used for this rejection."]
    #[must_use]
    pub fn body_text(&self) -> String {
        self.to_string()
    }
    #[doc = r" Get the status code used for this rejection."]
    #[must_use]
    pub fn status(&self) -> http::StatusCode {
        http::StatusCode::UNPROCESSABLE_ENTITY
    }
}
impl crate::response::IntoResponse for JsonDataError {
    fn into_response(self) -> crate::response::Response {
        let status = self.status();
        let body_text = self.body_text();
        (status, body_text).into_response()
    }
}
impl std::fmt::Display for JsonDataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Failed to deserialize the JSON body into the target type")?;
        f.write_str(": ")?;
        self.0.fmt(f)
    }
}
impl std::error::Error for JsonDataError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

define_rejection! {
    #[status = BAD_REQUEST]
    #[body = "Failed to parse the request body as JSON"]
    /// Rejection type for [`Json`](super::Json).
    ///
    /// This rejection is used if the request body didn't contain syntactically valid JSON.
    pub struct JsonSyntaxError(Error);
}

define_rejection! {
    #[status = UNSUPPORTED_MEDIA_TYPE]
    #[body = "Expected request with `Content-Type: application/json`"]
    /// Rejection type for [`Json`](super::Json) used if the `Content-Type`
    /// header is missing.
    pub struct MissingJsonContentType;
}

define_rejection! {
    #[status = INTERNAL_SERVER_ERROR]
    #[body = "Missing request extension"]
    /// Rejection type for [`Extension`](super::Extension) if an expected
    /// request extension was not found.
    pub struct MissingExtension(Error);
}

define_rejection! {
    #[status = INTERNAL_SERVER_ERROR]
    #[body = "No paths parameters found for matched route"]
    /// Rejection type used if axum's internal representation of path parameters
    /// is missing. This is commonly caused by extracting `Request<_>`. `Path`
    /// must be extracted first.
    pub struct MissingPathParams;
}

define_rejection! {
    #[status = UNSUPPORTED_MEDIA_TYPE]
    #[body = "Form requests must have `Content-Type: application/x-www-form-urlencoded`"]
    /// Rejection type for [`Form`](super::Form) or [`RawForm`](super::RawForm)
    /// used if the `Content-Type` header is missing
    /// or its value is not `application/x-www-form-urlencoded`.
    pub struct InvalidFormContentType;
}

define_rejection! {
    #[status = BAD_REQUEST]
    #[body = "Failed to deserialize form"]
    /// Rejection type used if the [`Form`](super::Form) extractor is unable to
    /// deserialize the form into the target type.
    pub struct FailedToDeserializeForm(Error);
}

define_rejection! {
    #[status = UNPROCESSABLE_ENTITY]
    #[body = "Failed to deserialize form body"]
    /// Rejection type used if the [`Form`](super::Form) extractor is unable to
    /// deserialize the form body into the target type.
    pub struct FailedToDeserializeFormBody(Error);
}

define_rejection! {
    #[status = BAD_REQUEST]
    #[body = "Failed to deserialize query string"]
    /// Rejection type used if the [`Query`](super::Query) extractor is unable to
    /// deserialize the query string into the target type.
    pub struct FailedToDeserializeQueryString(Error);
}

composite_rejection! {
    /// Rejection used for [`Query`](super::Query).
    ///
    /// Contains one variant for each way the [`Query`](super::Query) extractor
    /// can fail.
    pub enum QueryRejection {
        FailedToDeserializeQueryString,
    }
}

composite_rejection! {
    /// Rejection used for [`Form`](super::Form).
    ///
    /// Contains one variant for each way the [`Form`](super::Form) extractor
    /// can fail.
    pub enum FormRejection {
        InvalidFormContentType,
        FailedToDeserializeForm,
        FailedToDeserializeFormBody,
        BytesRejection,
    }
}

composite_rejection! {
    /// Rejection used for [`RawForm`](super::RawForm).
    ///
    /// Contains one variant for each way the [`RawForm`](super::RawForm) extractor
    /// can fail.
    pub enum RawFormRejection {
        InvalidFormContentType,
        BytesRejection,
    }
}

// composite_rejection! {
//     /// Rejection used for [`Json`](super::Json).
//     ///
//     /// Contains one variant for each way the [`Json`](super::Json) extractor
//     /// can fail.
//     #[cfg_attr(docsrs, doc(cfg(feature = "json")))]
//     pub enum JsonRejection {
//         JsonDataError,
//         JsonSyntaxError,
//         MissingJsonContentType,
//         BytesRejection,
//     }
// }

#[doc = r" Rejection used for [`Json`](super::Json)."]
#[doc = r""]
#[doc = r" Contains one variant for each way the [`Json`](super::Json) extractor"]
#[doc = r" can fail."]
#[cfg_attr(docsrs, doc(cfg(feature = "json")))]
#[derive(Debug)]
#[non_exhaustive]
pub enum JsonRejection {
    #[allow(missing_docs)]
    JsonDataError(JsonDataError),
    #[allow(missing_docs)]
    JsonSyntaxError(JsonSyntaxError),
    #[allow(missing_docs)]
    MissingJsonContentType(MissingJsonContentType),
    #[allow(missing_docs)]
    BytesRejection(BytesRejection),
}
impl crate::response::IntoResponse for JsonRejection {
    fn into_response(self) -> crate::response::Response {
        match self {
            Self::JsonDataError(inner) => inner.into_response(),
            Self::JsonSyntaxError(inner) => inner.into_response(),
            Self::MissingJsonContentType(inner) => inner.into_response(),
            Self::BytesRejection(inner) => inner.into_response(),
        }
    }
}
impl JsonRejection {
    #[doc = r" Get the response body text used for this rejection."]
    #[must_use]
    pub fn body_text(&self) -> String {
        match self {
            Self::JsonDataError(inner) => inner.body_text(),
            Self::JsonSyntaxError(inner) => inner.body_text(),
            Self::MissingJsonContentType(inner) => inner.body_text(),
            Self::BytesRejection(inner) => inner.body_text(),
        }
    }
    #[doc = r" Get the status code used for this rejection."]
    #[must_use]
    pub fn status(&self) -> http::StatusCode {
        match self {
            Self::JsonDataError(inner) => inner.status(),
            Self::JsonSyntaxError(inner) => inner.status(),
            Self::MissingJsonContentType(inner) => inner.status(),
            Self::BytesRejection(inner) => inner.status(),
        }
    }
}
impl From<JsonDataError> for JsonRejection {
    fn from(inner: JsonDataError) -> Self {
        Self::JsonDataError(inner)
    }
}
impl From<JsonSyntaxError> for JsonRejection {
    fn from(inner: JsonSyntaxError) -> Self {
        Self::JsonSyntaxError(inner)
    }
}
impl From<MissingJsonContentType> for JsonRejection {
    fn from(inner: MissingJsonContentType) -> Self {
        Self::MissingJsonContentType(inner)
    }
}
impl From<BytesRejection> for JsonRejection {
    fn from(inner: BytesRejection) -> Self {
        Self::BytesRejection(inner)
    }
}
impl std::fmt::Display for JsonRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::JsonDataError(inner) => f.write_fmt(core::format_args!("{inner}")),
            Self::JsonSyntaxError(inner) => f.write_fmt(core::format_args!("{inner}")),
            Self::MissingJsonContentType(inner) => f.write_fmt(core::format_args!("{inner}")),
            Self::BytesRejection(inner) => f.write_fmt(core::format_args!("{inner}")),
        }
    }
}
impl std::error::Error for JsonRejection {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::JsonDataError(inner) => inner.source(),
            Self::JsonSyntaxError(inner) => inner.source(),
            Self::MissingJsonContentType(inner) => inner.source(),
            Self::BytesRejection(inner) => inner.source(),
        }
    }
}

composite_rejection! {
    /// Rejection used for [`Extension`](super::Extension).
    ///
    /// Contains one variant for each way the [`Extension`](super::Extension) extractor
    /// can fail.
    pub enum ExtensionRejection {
        MissingExtension,
    }
}

composite_rejection! {
    /// Rejection used for [`Path`](super::Path).
    ///
    /// Contains one variant for each way the [`Path`](super::Path) extractor
    /// can fail.
    // TODO: add FailedToDeserializePathParams,
    pub enum PathRejection {
        MissingPathParams,
    }
}

composite_rejection! {
    /// Rejection used for [`RawPathParams`](super::RawPathParams).
    ///
    /// Contains one variant for each way the [`RawPathParams`](super::RawPathParams) extractor
    /// can fail.
    // TODO: add InvalidUtf8InPathParam,
    pub enum RawPathParamsRejection {
        MissingPathParams,
    }
}

define_rejection! {
    #[status = PAYLOAD_TOO_LARGE]
    #[body = "Failed to buffer the request body"]
    /// Encountered some other error when buffering the body.
    ///
    /// This can _only_ happen when you're using [`tower_http::limit::RequestBodyLimitLayer`] or
    /// otherwise wrapping request bodies in [`http_body_util::Limited`].
    pub struct LengthLimitError(Error);
}

define_rejection! {
    #[status = BAD_REQUEST]
    #[body = "Failed to buffer the request body"]
    /// Encountered an unknown error when buffering the body.
    pub struct UnknownBodyError(Error);
}

composite_rejection! {
    /// Rejection type for extractors that buffer the request body. Used if the
    /// request body cannot be buffered due to an error.
    pub enum FailedToBufferBody {
        LengthLimitError,
        UnknownBodyError,
    }
}

impl FailedToBufferBody {
    pub(crate) fn from_err<E>(err: E) -> Self
    where
        E: Into<BoxError>,
    {
        // two layers of boxes here because `with_limited_body`
        // wraps the `http_body_util::Limited` in a `axum_core::Body`
        // which also wraps the error type
        let box_error = match err.into().downcast::<Error>() {
            Ok(err) => err.into_inner(),
            Err(err) => err,
        };
        let box_error = match box_error.downcast::<Error>() {
            Ok(err) => err.into_inner(),
            Err(err) => err,
        };
        match box_error.downcast::<http_body_util::LengthLimitError>() {
            Ok(err) => Self::LengthLimitError(LengthLimitError::from_err(err)),
            Err(err) => Self::UnknownBodyError(UnknownBodyError::from_err(err)),
        }
    }
}

composite_rejection! {
    /// Rejection used for [`Bytes`](bytes::Bytes).
    ///
    /// Contains one variant for each way the [`Bytes`](bytes::Bytes) extractor
    /// can fail.
    pub enum BytesRejection {
        FailedToBufferBody,
    }
}

define_rejection! {
    #[status = INTERNAL_SERVER_ERROR]
    #[body = "No matched path found"]
    /// Rejection if no matched path could be found.
    ///
    /// See [`MatchedPath`](super::MatchedPath) for more details.
    #[cfg_attr(docsrs, doc(cfg(feature = "matched-path")))]
    pub struct MatchedPathMissing;
}

composite_rejection! {
    /// Rejection used for [`MatchedPath`](super::MatchedPath).
    #[cfg_attr(docsrs, doc(cfg(feature = "matched-path")))]
    pub enum MatchedPathRejection {
        MatchedPathMissing,
    }
}

define_rejection! {
    #[status = INTERNAL_SERVER_ERROR]
    #[body = "The matched route is not nested"]
    /// Rejection type for [`NestedPath`](super::NestedPath).
    ///
    /// This rejection is used if the matched route wasn't nested.
    pub struct NestedPathRejection;
}
