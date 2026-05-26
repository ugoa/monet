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
        layer: Layer,
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
    F: 'static + Fn(Request, Layer) -> Fut,
    Fut: Future<Output = Resp>,
    Resp: IntoResponse,
{
    fn transform(
        &self,
        req: Request,
        layer: Layer,
    ) -> Pin<Box<dyn Future<Output = Response> + '_>> {
        Box::pin(async move { (self)(req, layer).await.into_response() })
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

#[derive(Clone, Debug)]
pub struct Layer {
    pub(crate) middlewares: Vec<Rc<dyn Middleware>>,
    pub(crate) endpoint: Rc<dyn Endpoint>,
}

impl Layer {
    pub fn new(endpoint: impl Endpoint) -> Self {
        Layer {
            middlewares: Default::default(),
            endpoint: Rc::new(endpoint),
        }
    }

    pub fn append(&mut self, m: Rc<impl Middleware>) {
        self.middlewares.push(m.clone());
    }

    pub async fn next(mut self, req: Request) -> Response {
        if let Some(current) = self.middlewares.pop() {
            current.transform(req, self).await
        } else {
            self.endpoint.call(req).await
        }
    }
}
