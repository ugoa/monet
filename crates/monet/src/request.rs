use std::{
    any::{Any, TypeId},
    collections::HashMap,
    hash::BuildHasherDefault,
};

use http::{HeaderMap, HeaderValue, Method, Uri, Version};
use hyper::{Request as HttpRequest, Response as HttpResponse, body::Incoming as IncomingBody};

pub struct Request {
    head: Parts,
    body: IncomingBody,
    state: State,
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
    pub fn state(&self) -> &State {
        &self.state
    }

    #[inline]
    pub fn state_mut(&mut self) -> &mut State {
        &mut self.state
    }
}

impl From<HttpRequest<IncomingBody>> for Request {
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
            body,
            state: State { map: None },
        }
    }
}

#[derive(Default)]
struct IdHasher(u64);

#[derive(Clone)]
pub struct State {
    map: Option<Box<HashMap<TypeId, Box<dyn AnyClone>, BuildHasherDefault<IdHasher>>>>,
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
