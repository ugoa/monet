use serde::de::DeserializeOwned;

use crate::{
    extract::rejection::{JsonDataError, JsonRejection, JsonSyntaxError},
    json::Json,
};

impl<T> Json<T>
where
    T: DeserializeOwned,
{
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, JsonRejection> {
        let mut deserializer = serde_json::Deserializer::from_slice(bytes);

        serde_path_to_error::deserialize(&mut deserializer)
            .map_err(error_to_rejection)
            .and_then(|value| {
                deserializer
                    .end()
                    .map(|()| Self(value))
                    .map_err(|err| JsonSyntaxError::from_err(err).into())
            })
    }
}

fn error_to_rejection(err: serde_path_to_error::Error<serde_json::Error>) -> JsonRejection {
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
