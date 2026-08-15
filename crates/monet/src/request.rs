use std::{
    any::{Any, TypeId},
    collections::HashMap,
    hash::{BuildHasherDefault, Hasher},
    marker::PhantomData,
    rc::Rc,
};

use bytes::Bytes;
use http::{HeaderMap, HeaderValue, Method, Uri, Version};
use http_body_util::BodyExt;
use hyper::body::Incoming as IncomingBody;
use serde_core::de::DeserializeOwned;

use crate::{
    body::Body,
    error::Error,
    router::url::UrlParams,
    types::{Form, Json, Path, Query, has_content_type},
};

// Custom Parts to remove the Extension due to its Send + Sync bound
// Instead, we use State which can store both Send and non-Send data
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

pub struct Request {
    pub body: Body,
    pub head: Parts,
    pub extensions: Extensions,
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

    pub fn path<T>(&self) -> Result<Path<T>, Error>
    where
        T: DeserializeOwned,
    {
        /*
         * Given route: `/user/{id}/{name}` and request path: `/user/23/david`
         * The data transformation would be:
         *
         *    Vec[("id", "23"), ("name", "mike")]
         *      -> id=23&name=mike
         *      -> Path(T {id: 23, name: mike})
         */
        match self.state::<UrlParams>() {
            Some(UrlParams::PairParams(params)) => {
                let mut serializer = form_urlencoded::Serializer::new(String::new());
                params.iter().for_each(|(k, v)| {
                    serializer.append_pair(k, v);
                });
                let encoded_querystring = serializer.finish();

                let parser = form_urlencoded::parse(encoded_querystring.as_bytes());
                let deserializer = serde_urlencoded::Deserializer::new(parser);
                serde_path_to_error::deserialize(deserializer)
                    .map(Path)
                    .map_err(Error::FailedToDeserializePathParams)
            }
            Some(UrlParams::InvalidUtf8Param { key }) => Err(Error::InvalidUtf8InPathParam {
                key: key.to_string(),
            }),
            None => Err(Error::MissingPathParams),
        }
    }

    pub fn query_params<T>(&self) -> Result<Query<T>, Error>
    where
        T: DeserializeOwned,
    {
        let query = self.uri().query().unwrap_or_default();
        let parser = form_urlencoded::parse(query.as_bytes());
        let deserializer = serde_urlencoded::Deserializer::new(parser);
        serde_path_to_error::deserialize(deserializer)
            .map(Query)
            .map_err(Error::FailedToDeserializeQuery)
    }

    // #[cfg(not(feature = "no-matched-path"))]
    pub fn matched_path(&self) -> Option<&Rc<str>> {
        use crate::router::url::MatchedPath;

        self.state::<MatchedPath>().map(|s| &s.0)
    }

    pub fn raw_query(&self) -> Option<String> {
        self.uri().query().map(|query| query.to_owned())
    }

    pub async fn into_form<T>(self) -> Result<Form<T>, Error>
    where
        T: DeserializeOwned,
    {
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
                return Err(Error::InvalidFormContentType);
            }
        };

        let deserializer = serde_html_form::Deserializer::new(form_urlencoded::parse(&bytes));
        serde_path_to_error::deserialize(deserializer).map_err(Error::FailedToDeserializeForm)
    }

    pub async fn into_bytes(self) -> Result<Bytes, Error> {
        Ok(self.body.collect().await?.to_bytes())
    }

    pub async fn into_json<T>(self) -> Result<Json<T>, Error>
    where
        T: DeserializeOwned,
    {
        if !has_content_type(self.headers(), &mime::APPLICATION_JSON) {
            return Err(Error::InvalidJsonContentType);
        }

        let bytes = &self.into_bytes().await?;
        let mut deserializer = serde_json::Deserializer::from_slice(bytes);
        match serde_path_to_error::deserialize(&mut deserializer) {
            Ok(value) => match deserializer.end() {
                Ok(()) => Ok(Json(value)),
                Err(err) => Err(Error::JsonSyntaxError(err)),
            },
            Err(err) => Err(Error::JsonDataError(err)),
        }
    }

    pub fn state<T: 'static>(&self) -> Option<&T> {
        self.extensions.get()
    }

    pub fn state_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.extensions.get_mut()
    }

    pub fn add_state<T: Clone + 'static>(&mut self, val: T) -> Option<T> {
        self.extensions.insert(val)
    }

    pub fn remove_state<T: 'static>(&mut self) -> Option<T> {
        self.extensions.remove()
    }
}

impl From<http::Request<IncomingBody>> for Request {
    fn from(http_req: http::Request<IncomingBody>) -> Self {
        let (parts, body) = http_req.into_parts();

        Self {
            head: Parts {
                method: parts.method,
                version: parts.version,
                uri: parts.uri,
                headers: parts.headers,
            },
            body: Body::new(body),
            extensions: Default::default(),
        }
    }
}

type AnyCloneMap = HashMap<TypeId, Box<dyn AnyClone>, BuildHasherDefault<IdHasher>>;

#[derive(Clone, Default)]
pub struct Extensions<'a> {
    pub map: HashMap<TypeId, Box<dyn AnyClone>, BuildHasherDefault<IdHasher>>,
    _marker: PhantomData<&'a ()>,
}

impl<'a> Extensions<'a> {
    pub fn insert<T: 'static>(&mut self, val: &'a T) -> Option<T> {
        // SAFETY: `AnyClone` require 'static, so we lie to it by erasing 'a → 'static.
        // This is safe because PhantomData<&'a ()> guarantees
        // the State cannot outlive 'a, so the reference to T is
        // always valid during the State's existence.
        let erased: &'static T = unsafe { std::mem::transmute(val) };
        self.map
            .insert(TypeId::of::<&T>(), Box::new(erased))
            .and_then(|boxed| boxed.into_any().downcast().ok().map(|boxed| *boxed))
    }

    fn get<T: 'static>(&self) -> Option<&'a T> {
        self.map
            .get(&TypeId::of::<&T>())
            .and_then(|boxed: &Box<dyn AnyClone>| {
                (**boxed)
                    .as_any()
                    .downcast_ref()
                    .map(|s: &T| unsafe { std::mem::transmute(s) })
            })
    }

    fn get_mut<T: 'static>(&mut self) -> Option<&'a mut T> {
        self.map
            .get_mut(&TypeId::of::<&T>())
            .and_then(|boxed: &mut Box<dyn AnyClone + 'static>| {
                (**boxed)
                    .as_any_mut()
                    .downcast_mut()
                    .map(|s: &mut T| unsafe { std::mem::transmute(s) })
            })
    }

    pub fn remove<T: 'static>(&mut self) -> Option<&'a T> {
        self.map
            .remove(&TypeId::of::<&T>())
            .and_then(|boxed: Box<dyn AnyClone + 'static>| {
                boxed
                    .into_any()
                    .downcast::<&'static T>()
                    .ok()
                    .map(|boxed: Box<&'static T>| *boxed)
                    .map(|val: &'static T| unsafe { std::mem::transmute(val) })
            })
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
