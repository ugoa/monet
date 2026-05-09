use std::{
    any::{Any, TypeId},
    collections::HashMap,
    hash::{BuildHasherDefault, Hasher},
};

use bytes::Bytes;
use http::{HeaderMap, HeaderValue, Method, Uri, Version};
use http_body_util::BodyExt;
use hyper::body::Incoming as IncomingBody;
use serde_core::de::DeserializeOwned;

use crate::{
    body::Body,
    extract::{
        has_content_type,
        rejection::{
            BytesRejection, FailedToBufferBody, FailedToDeserializeForm,
            FailedToDeserializeFormBody, FailedToDeserializeQueryString, FormRejection,
            InvalidFormContentType, JsonRejection, MissingJsonContentType, QueryRejection,
        },
    },
    form::Form,
    json::Json,
};

pub struct Request {
    pub body: Body,
    pub state: State,
    head: Parts,
}

impl Request {
    #[inline]
    pub fn method(&self) -> &Method {
        &self.head.method
    }

    #[inline]
    pub fn method_mut(&mut self) -> &mut Method {
        &mut self.head.method
    }

    #[inline]
    pub fn uri(&self) -> &Uri {
        &self.head.uri
    }

    #[inline]
    pub fn uri_mut(&mut self) -> &mut Uri {
        &mut self.head.uri
    }

    #[inline]
    pub fn version(&self) -> &Version {
        &self.head.version
    }

    #[inline]
    pub fn version_mut(&mut self) -> &mut Version {
        &mut self.head.version
    }

    #[inline]
    pub fn headers(&self) -> &HeaderMap {
        &self.head.headers
    }

    #[inline]
    pub fn headers_mut(&mut self) -> &mut HeaderMap {
        &mut self.head.headers
    }

    pub fn query<T: DeserializeOwned>(&self) -> Result<T, QueryRejection> {
        let query = self.uri().query().unwrap_or_default();

        let deserializer =
            serde_urlencoded::Deserializer::new(form_urlencoded::parse(query.as_bytes()));
        let params = serde_path_to_error::deserialize(deserializer)
            .map_err(FailedToDeserializeQueryString::from_err)?;
        Ok(params)
    }

    pub async fn into_form<T: DeserializeOwned>(self) -> Result<Form<T>, FormRejection> {
        let is_get_or_head =
            self.method() == http::Method::GET || self.method() == http::Method::HEAD;
        let bytes = if self.method() == Method::GET {
            if let Some(query) = self.uri().query() {
                Bytes::copy_from_slice(query.as_bytes())
            } else {
                Bytes::new()
            }
        } else {
            if has_content_type(self.headers(), &mime::APPLICATION_WWW_FORM_URLENCODED) {
                self.into_bytes().await?
            } else {
                return Err(InvalidFormContentType.into());
            }
        };

        let deserializer = serde_urlencoded::Deserializer::new(form_urlencoded::parse(&bytes));
        let value: Form<T> =
            serde_path_to_error::deserialize(deserializer).map_err(|err| -> FormRejection {
                if is_get_or_head {
                    FailedToDeserializeForm::from_err(err).into()
                } else {
                    FailedToDeserializeFormBody::from_err(err).into()
                }
            })?;
        Ok(value)
    }

    pub async fn into_bytes(self) -> Result<Bytes, BytesRejection> {
        let bytes = self
            .with_limited_body()
            .body
            .collect()
            .await
            .map_err(FailedToBufferBody::from_err)?
            .to_bytes();

        Ok(bytes)
    }

    pub async fn into_json<T: DeserializeOwned>(self) -> Result<Json<T>, JsonRejection> {
        if has_content_type(self.headers(), &mime::APPLICATION_JSON) {
            Json::from_bytes(&self.into_bytes().await?)
        } else {
            Err(MissingJsonContentType.into())
        }
    }

    // TODO: https://github.com/ugoa/axum/blob/061666a1116d853f9ca838fb2d0c668614a9f535/axum-core/src/ext_traits/request.rs?plain=1#L316
    fn with_limited_body(self) -> Request {
        self
    }
}

impl From<http::Request<IncomingBody>> for Request {
    fn from(http_req: http::Request<IncomingBody>) -> Self {
        let (
            http::request::Parts {
                method,
                uri,
                version,
                headers,
                ..
            },
            body,
        ) = http_req.into_parts();

        Self {
            head: Parts {
                method,
                uri,
                version,
                headers,
            },
            body: Body::new(body),
            state: State { map: None },
        }
    }
}

#[derive(Clone, Default)]
pub struct State {
    map: Option<Box<HashMap<TypeId, Box<dyn AnyClone>, BuildHasherDefault<IdHasher>>>>,
}

impl State {
    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.map
            .as_ref()
            .and_then(|map| map.get(&TypeId::of::<T>()))
            .and_then(|boxed| (**boxed).as_any().downcast_ref())
    }

    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.map
            .as_mut()
            .and_then(|map| map.get_mut(&TypeId::of::<T>()))
            .and_then(|boxed| (**boxed).as_any_mut().downcast_mut())
    }

    pub fn insert<T: Clone + 'static>(&mut self, val: T) -> Option<T> {
        self.map
            .get_or_insert_with(Box::default)
            .insert(TypeId::of::<T>(), Box::new(val))
            .and_then(|boxed| boxed.into_any().downcast().ok().map(|boxed| *boxed))
    }

    pub fn remove<T: 'static>(&mut self) -> Option<T> {
        self.map
            .as_mut()
            .and_then(|map| map.remove(&TypeId::of::<T>()))
            .and_then(|boxed| boxed.into_any().downcast().ok().map(|boxed| *boxed))
    }

    #[inline]
    pub fn clear(&mut self) {
        if let Some(ref mut map) = self.map {
            map.clear();
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.map.as_ref().map_or(true, |map| map.is_empty())
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.map.as_ref().map_or(0, |map| map.len())
    }
}

#[derive(Default)]
struct IdHasher(u64);

impl Hasher for IdHasher {
    fn write(&mut self, _: &[u8]) {
        unreachable!("TypeId calls write_u64");
    }

    #[inline]
    fn write_u64(&mut self, id: u64) {
        self.0 = id;
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
}

#[derive(Clone)]
struct Parts {
    /// The request's method
    method: Method,

    /// The request's URI
    uri: Uri,

    /// The request's version
    version: Version,

    /// The request's headers
    headers: HeaderMap<HeaderValue>,
}

pub(crate) trait AnyClone: Any {
    fn clone_box(&self) -> Box<dyn AnyClone>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

impl<T: Clone + 'static> AnyClone for T {
    fn clone_box(&self) -> Box<dyn AnyClone> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

impl Clone for Box<dyn AnyClone> {
    fn clone(&self) -> Self {
        (**self).clone_box()
    }
}
