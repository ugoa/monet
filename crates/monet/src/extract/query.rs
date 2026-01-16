use std::convert::Infallible;

use http::request::Parts;
use serde_core::de::DeserializeOwned;

use crate::extract::FromRequestParts;

#[derive(Debug, Clone, Copy, Default)]
pub struct Query<T>(pub T);

impl<T> FromRequestParts for Query<T>
where
    T: DeserializeOwned,
{
    type Rejection = Infallible; // Todo handle invalid query string

    async fn from_request_parts(parts: &mut Parts) -> Result<Self, Self::Rejection> {
        let query = parts.uri.query().unwrap_or_default();
        let deserializer =
            serde_urlencoded::Deserializer::new(form_urlencoded::parse(query.as_bytes()));
        let params = serde_path_to_error::deserialize(deserializer).unwrap();
        // .map_err(FailedToDeserializeQueryString::from_err)?;
        Ok(Self(params))
    }
}
