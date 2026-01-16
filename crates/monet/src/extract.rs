use std::convert::Infallible;

use crate::{HttpRequest, response::IntoResponse};
use http::{Method, Uri, request::Parts};

// pub mod state;

pub mod query;

#[derive(Debug, Clone, Copy)]
pub enum ViaParts {}

#[derive(Debug, Clone, Copy)]
pub enum ViaRequest {}

pub trait FromRequest<M = ViaRequest>: Sized {
    /// If the extractor fails it'll use this "rejection" type. A rejection is
    /// a kind of error that can be converted into a response.
    type Rejection: IntoResponse;

    /// Perform the extraction.
    async fn from_request(req: HttpRequest) -> Result<Self, Self::Rejection>;
}

impl<T> FromRequest<ViaParts> for T
where
    T: FromRequestParts,
{
    type Rejection = <Self as FromRequestParts>::Rejection;

    async fn from_request(req: HttpRequest) -> Result<Self, Self::Rejection> {
        let (mut parts, _) = req.into_parts();
        Self::from_request_parts(&mut parts).await
    }
}

impl<T> FromRequest for Result<T, T::Rejection>
where
    T: FromRequest,
{
    type Rejection = Infallible;

    async fn from_request(req: HttpRequest) -> Result<Self, Self::Rejection> {
        Ok(T::from_request(req).await)
    }
}

pub trait FromRequestParts: Sized {
    /// If the extractor fails it'll use this "rejection" type. A rejection is
    /// a kind of error that can be converted into a response.
    type Rejection: IntoResponse;

    /// Perform the extraction.
    async fn from_request_parts(parts: &mut Parts) -> Result<Self, Self::Rejection>;
}

impl<T> FromRequestParts for Result<T, T::Rejection>
where
    T: FromRequestParts,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts) -> Result<Self, Self::Rejection> {
        Ok(T::from_request_parts(parts).await)
    }
}

impl FromRequestParts for Method {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts) -> Result<Self, Self::Rejection> {
        Ok(parts.method.clone())
    }
}

impl FromRequestParts for Uri {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts) -> Result<Self, Self::Rejection> {
        Ok(parts.uri.clone())
    }
}
