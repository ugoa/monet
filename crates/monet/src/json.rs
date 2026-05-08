use std::convert::Infallible;

use http::HeaderMap;
use serde_core::de::DeserializeOwned;
use serde_json::Error;

#[derive(Debug, Clone, Copy, Default)]
#[must_use]
pub struct Json<T>(pub T);

impl<T> Json<T>
where
    T: DeserializeOwned,
{
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut deserializer = serde_json::Deserializer::from_slice(bytes);

        let s = serde_path_to_error::deserialize(&mut deserializer)
            .and_then(|value| {
                let _ = deserializer.end();
                Ok(Self(value))
            })
            .unwrap();
        s
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
