use serde::de::DeserializeOwned;
use serde_json::error::Category as CatError;

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

        match serde_path_to_error::deserialize(&mut deserializer) {
            Ok(value) => match deserializer.end() {
                Ok(()) => Ok(Self(value)),
                Err(err) => Err(JsonSyntaxError::from_err(err).into()),
            },
            Err(err) => match err.inner().classify() {
                CatError::Data => Err(JsonDataError::from_err(err).into()),
                CatError::Syntax | CatError::Eof => Err(JsonSyntaxError::from_err(err).into()),
                CatError::Io => Err(JsonSyntaxError::from_err(err).into()),
            },
        }
    }
}
