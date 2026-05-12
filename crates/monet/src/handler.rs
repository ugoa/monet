use std::rc::Rc;

use async_trait::async_trait;

use crate::{
    request::Request,
    response::{IntoResponse, Response},
};

pub mod endpoint;

#[async_trait(?Send)]
pub trait Middleware: 'static {
    async fn transform(&self, request: Request, chain: Chain) -> Response;

    /// Set the middleware's name. By default it uses the type signature.
    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }
}

impl std::fmt::Debug for dyn Middleware {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Endpoint: {{{}}}", self.name())
    }
}

#[async_trait(?Send)]
impl<F, Fut, Resp> Middleware for F
where
    F: 'static + Fn(Request, Chain) -> Fut,
    Fut: Future<Output = Resp>,
    Resp: IntoResponse,
{
    async fn transform(&self, req: Request, chain: Chain) -> Response {
        (self)(req, chain).await.into_response()
    }
}

#[async_trait(?Send)]
pub trait Endpoint: 'static {
    async fn call(&self, req: Request) -> Response;

    /// Set the middleware's name. By default it uses the type signature.
    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }
}

impl std::fmt::Debug for dyn Endpoint {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Endpoint: {{{}}}", self.name())
    }
}

#[async_trait(?Send)]
impl<F, Fut, Resp> Endpoint for F
where
    F: 'static + Fn(Request) -> Fut,
    Fut: Future<Output = Resp>,
    Resp: IntoResponse,
{
    async fn call(&self, req: Request) -> Response {
        (self)(req).await.into_response()
    }
}

#[derive(Clone, Debug)]
pub struct Chain {
    pub(crate) endpoint: Rc<dyn Endpoint>,
    pub(crate) middlewares: Vec<Rc<dyn Middleware>>,
}

impl Chain {
    pub async fn next(mut self, req: Request) -> Response {
        if let Some(current) = self.middlewares.pop() {
            current.transform(req, self).await
        } else {
            self.endpoint.call(req).await
        }
    }
}
