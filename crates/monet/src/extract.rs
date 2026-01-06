use std::convert::Infallible;

use crate::{HttpRequest, response::IntoResponse};
use http::{Method, Uri, request::Parts};

pub mod state;

pub mod query;

#[derive(Debug, Clone, Copy)]
pub enum ViaParts {}

#[derive(Debug, Clone, Copy)]
pub enum ViaRequest {}

pub trait FromRequest<'a, S, M = ViaRequest>: Sized {
    /// If the extractor fails it'll use this "rejection" type. A rejection is
    /// a kind of error that can be converted into a response.
    type Rejection: IntoResponse<'a>;

    /// Perform the extraction.
    async fn from_request(req: HttpRequest, state: &S) -> Result<Self, Self::Rejection>;
}

impl<'a, S, T> FromRequest<'a, S, ViaParts> for T
where
    T: FromRequestParts<'a, S>,
{
    type Rejection = <Self as FromRequestParts<S>>::Rejection;

    async fn from_request(req: HttpRequest<'_>, state: &S) -> Result<Self, Self::Rejection> {
        let (mut parts, _) = req.into_parts();
        Self::from_request_parts(&mut parts, state).await
    }
}

impl<'a, S, T> FromRequest<'a, S> for Result<T, T::Rejection>
where
    T: FromRequest<'a, S>,
{
    type Rejection = Infallible;

    async fn from_request(req: HttpRequest<'_>, state: &S) -> Result<Self, Self::Rejection> {
        Ok(T::from_request(req, state).await)
    }
}

pub trait FromRequestParts<'a, S>: Sized {
    /// If the extractor fails it'll use this "rejection" type. A rejection is
    /// a kind of error that can be converted into a response.
    type Rejection: IntoResponse<'a>;

    /// Perform the extraction.
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection>;
}

impl<'a, S, T> FromRequestParts<'a, S> for Result<T, T::Rejection>
where
    T: FromRequestParts<'a, S>,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Ok(T::from_request_parts(parts, state).await)
    }
}

impl<S> FromRequestParts<'_, S> for Method {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Ok(parts.method.clone())
    }
}

impl<S> FromRequestParts<'_, S> for Uri {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Ok(parts.uri.clone())
    }
}
