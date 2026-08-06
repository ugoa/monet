pub mod endpoint;
pub mod middleware;

use std::{pin::Pin, rc::Rc};

use crate::{
    request::Request,
    response::{IntoResponse, Response},
};

pub trait Middleware: 'static {
    #[must_use]
    fn transform(
        &self,
        request: Request,
        chain: Chain,
    ) -> Pin<Box<dyn Future<Output = Response> + '_>>;

    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }
}

impl std::fmt::Debug for dyn Middleware {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Endpoint: {{{}}}", self.name())
    }
}

impl<F, Fut, Resp> Middleware for F
where
    F: 'static + Fn(Request, Chain) -> Fut,
    Fut: Future<Output = Resp>,
    Resp: IntoResponse,
{
    fn transform(
        &self,
        req: Request,
        chain: Chain,
    ) -> Pin<Box<dyn Future<Output = Response> + '_>> {
        Box::pin(async move { (self)(req, chain).await.into_response() })
    }
}

pub trait Endpoint: 'static {
    fn call(&self, req: Request) -> Pin<Box<dyn Future<Output = Response> + '_>>;

    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }
}

impl std::fmt::Debug for dyn Endpoint {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Endpoint: {{{}}}", self.name())
    }
}

impl<F, Fut, Resp> Endpoint for F
where
    F: 'static + Fn(Request) -> Fut,
    Fut: Future<Output = Resp>,
    Resp: IntoResponse,
{
    fn call(&self, req: Request) -> Pin<Box<dyn Future<Output = Response> + '_>> {
        Box::pin(async move { (self)(req).await.into_response() })
    }
}

#[derive(Debug, Clone)]
pub struct Chain {
    pub(crate) middlewares: Vec<Rc<dyn Middleware>>,
    pub(crate) endpoint: Rc<dyn Endpoint>,
}

impl Chain {
    pub async fn next(mut self, req: Request) -> Response {
        if let Some(mw) = self.middlewares.pop() {
            mw.transform(req, self).await
        } else {
            self.endpoint.call(req).await
        }
    }

    pub(crate) fn new(endpoint: impl Endpoint) -> Self {
        Chain {
            middlewares: Default::default(),
            endpoint: Rc::new(endpoint),
        }
    }

    pub(crate) fn append(&mut self, m: Rc<impl Middleware>) {
        self.middlewares.push(m.clone());
    }
}
