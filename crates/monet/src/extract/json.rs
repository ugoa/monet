use serde::de::DeserializeOwned;
use serde_json::error::Category as CatError;

use crate::{error::Error, json::Json};
impl<T> Json<T>
where
    T: DeserializeOwned,
{
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        let mut deserializer = serde_json::Deserializer::from_slice(bytes);

        match serde_path_to_error::deserialize(&mut deserializer) {
            Ok(value) => match deserializer.end() {
                Ok(()) => Ok(Self(value)),
                Err(err) => Err(Error::JsonSyntaxError(err)),
            },
            Err(err) => match err.inner().classify() {
                CatError::Data => Err(Error::JsonDataError(err)),
                CatError::Syntax | CatError::Eof => Err(Error::JsonDataError(err)),
                CatError::Io => Err(Error::JsonDataError(err)),
            },
        }
    }
}
