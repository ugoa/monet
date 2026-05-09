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
    json::{Json, json_content_type},
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

    pub async fn into_bytes(self) -> Bytes {
        let bytes = self
            .with_limited_body()
            .body
            .collect()
            .await
            .unwrap()
            .to_bytes();
        bytes
    }

    // TODO: change to Result
    #[inline]
    pub async fn into_json<T: DeserializeOwned>(self) -> Option<Json<T>> {
        if json_content_type(self.headers()) {
            Some(Json::from_bytes(&self.into_bytes().await))
        } else {
            None
        }
    }

    fn with_limited_body(self) -> Request {
        // // update docs in `axum-core/src/extract/default_body_limit.rs` and
        // // `axum/src/docs/extract.md` if this changes
        // const DEFAULT_LIMIT: usize = 2_097_152; // 2 mb
        //
        // match self.extensions().get::<DefaultBodyLimitKind>().copied() {
        //     Some(DefaultBodyLimitKind::Disable) => self,
        //     Some(DefaultBodyLimitKind::Limit(limit)) => {
        //         self.map(|b| Body::new(http_body_util::Limited::new(b, limit)))
        //     }
        //     None => self.map(|b| Body::new(http_body_util::Limited::new(b, DEFAULT_LIMIT))),
        // }
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
