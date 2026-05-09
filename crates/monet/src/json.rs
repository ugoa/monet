use std::convert::Infallible;

use http::HeaderMap;
use serde_core::de::DeserializeOwned;
use serde_json::Error;

use crate::extract::rejection::{
    BytesRejection, JsonDataError, JsonRejection, JsonSyntaxError, MissingJsonContentType,
};

#[derive(Debug, Clone, Copy, Default)]
#[must_use]
pub struct Json<T>(pub T);

impl<T> Json<T>
where
    T: DeserializeOwned,
{
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, JsonRejection> {
        let mut deserializer = serde_json::Deserializer::from_slice(bytes);

        serde_path_to_error::deserialize(&mut deserializer)
            .map_err(make_rejection)
            .and_then(|value| {
                deserializer
                    .end()
                    .map(|()| Self(value))
                    .map_err(|err| JsonSyntaxError::from_err(err).into())
            })
    }
}

fn make_rejection(err: serde_path_to_error::Error<serde_json::Error>) -> JsonRejection {
    match err.inner().classify() {
        serde_json::error::Category::Data => JsonDataError::from_err(err).into(),
        serde_json::error::Category::Syntax | serde_json::error::Category::Eof => {
            JsonSyntaxError::from_err(err).into()
        }
        serde_json::error::Category::Io => {
            if cfg!(debug_assertions) {
                // we don't use `serde_json::from_reader` and instead always buffer
                // bodies first, so we shouldn't encounter any IO errors
                unreachable!()
            } else {
                JsonSyntaxError::from_err(err).into()
            }
        }
    }
}

impl<T> std::ops::Deref for Json<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T> std::ops::DerefMut for Json<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub(crate) fn json_content_type(headers: &HeaderMap) -> bool {
    headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|content_type| content_type.to_str().ok())
        .and_then(|content_type| content_type.parse::<mime::Mime>().ok())
        .is_some_and(|mime| {
            mime.type_() == "application"
                && (mime.subtype() == "json" || mime.suffix().is_some_and(|name| name == "json"))
        })
}
