use std::{
    any::{Any, TypeId},
    collections::HashMap,
    hash::{BuildHasherDefault, Hasher},
};

use http::{HeaderMap, HeaderValue, Method, Uri, Version};
use hyper::{Request as HttpRequest, body::Incoming as IncomingBody};

use crate::body::Body;

pub struct Request {
    pub(crate) head: Parts,
    pub body: IncomingBody,
    pub state: State,
}

pub struct NewRequest {
    pub(crate) head: Parts,
    pub body: Body,
    pub state: State,
}

impl NewRequest {
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
}

impl From<HttpRequest<IncomingBody>> for NewRequest {
    fn from(http_req: HttpRequest<IncomingBody>) -> Self {
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
pub struct Parts {
    /// The request's method
    pub method: Method,

    /// The request's URI
    pub uri: Uri,

    /// The request's version
    pub version: Version,

    /// The request's headers
    pub headers: HeaderMap<HeaderValue>,
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
