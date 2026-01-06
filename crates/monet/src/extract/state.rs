use std::{
    convert::Infallible,
    ops::{Deref, DerefMut},
};

use crate::extract::FromRequestParts;

#[derive(Debug, Default, Clone, Copy)]
pub struct State<S>(pub S);

impl<S> Deref for State<S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<S> DerefMut for State<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<S> FromRequestParts<'_, S> for State<S>
where
    S: Clone,
{
    type Rejection = Infallible;

    async fn from_request_parts(
        _parts: &mut http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        Ok(Self(state.clone()))
    }
}
