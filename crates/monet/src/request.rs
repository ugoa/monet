use std::{
    any::{Any, TypeId},
    collections::HashMap,
    hash::{BuildHasherDefault, Hasher},
    rc::Rc,
};

use bytes::Bytes;
use http::{HeaderMap, HeaderValue, Method, Uri, Version};
use http_body_util::BodyExt;
use hyper::body::Incoming as IncomingBody;
use serde_core::de::DeserializeOwned;
use smallvec::SmallVec;

use crate::{
    body::Body,
    error::Error,
    router::url::UrlParams,
    types::{Form, Json, Path, Query, has_content_type},
};

pub struct Request {
    head: Parts,
    body: Body,
    data: SmallVec<[Rc<State>; 4]>,
}

// Custom Parts to remove the Extension due to its Send + Sync bound
// Instead, we use State which can store both Send and non-Send data
#[derive(Clone)]
pub struct Parts {
    method: Method,
    uri: Uri,
    version: Version,
    headers: HeaderMap<HeaderValue>,
}

#[derive(Default)]
pub struct State(AnyMap);

type AnyMap = HashMap<TypeId, Box<dyn Any>, BuildHasherDefault<IdHasher>>;

impl Request {
    pub fn state<T: 'static>(&self) -> Option<&T> {
        for container in self.data.iter().rev() {
            if let Some(data) = container.get::<T>() {
                return Some(data);
            }
        }

        None
    }

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
         * Given route: `/user/{id}/{*name}`
         * and request: `/user/23/david`
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

    pub fn query<T>(&self) -> Result<Query<T>, Error>
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
            data: Default::default(),
        }
    }
}

impl State {
    pub fn insert<T: 'static>(&mut self, val: T) -> Option<T> {
        self.0
            .insert(TypeId::of::<T>(), Box::new(val))
            .and_then(|boxed| boxed.downcast().ok().map(|boxed| *boxed))
    }

    pub fn remove<T: 'static>(&mut self) -> Option<T> {
        self.0
            .remove(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast().ok().map(|boxed| *boxed))
    }

    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.0
            .get(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_ref())
    }

    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.0
            .get_mut(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_mut())
    }

    pub fn get_or_insert_with<T: 'static, F: FnOnce() -> T>(&mut self, default: F) -> &mut T {
        self.0
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(default()))
            .downcast_mut()
            .expect("state shall now contain a T value")
    }

    pub fn contains<T: 'static>(&self) -> bool {
        self.0.contains_key(&TypeId::of::<T>())
    }

    #[inline]
    pub fn clear(&mut self) {
        self.0.clear();
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
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
