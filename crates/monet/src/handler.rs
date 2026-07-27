pub mod endpoint;
pub mod middleware;

use std::{pin::Pin, rc::Rc};

use dyn_clone::DynClone;

use crate::{
    request::Request,
    response::{IntoResponse, Response},
};

pub trait Middleware: DynClone + 'static {
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

impl<'cl> Clone for Box<dyn Middleware + 'cl> {
    fn clone(&self) -> Self {
        dyn_clone::clone_box(&**self)
    }
}

impl std::fmt::Debug for dyn Middleware {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Endpoint: {{{}}}", self.name())
    }
}

impl<F, Fut, Resp> Middleware for F
where
    F: 'static + Clone + Fn(Request, Layer) -> Fut,
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

pub trait Endpoint: DynClone + 'static {
    fn call(&self, req: Request) -> Pin<Box<dyn Future<Output = Response> + '_>>;

    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }
}

/// Handwriten Version of:
///     dyn_clone::clone_trait_object!(Endpoint);
/// without the redundant Send&Sync implemention
impl<'cl> Clone for Box<dyn Endpoint + 'cl> {
    fn clone(&self) -> Self {
        dyn_clone::clone_box(&**self)
    }
}

impl std::fmt::Debug for dyn Endpoint {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Endpoint: {{{}}}", self.name())
    }
}

impl<F, Fut, Resp> Endpoint for F
where
    F: 'static + Clone + Fn(Request) -> Fut,
    Fut: Future<Output = Resp>,
    Resp: IntoResponse,
{
    fn call(&self, req: Request) -> Pin<Box<dyn Future<Output = Response> + '_>> {
        Box::pin(async move { (self)(req).await.into_response() })
    }
}

#[derive(Debug)]
pub struct Layer {
    pub(crate) middlewares: Vec<Rc<dyn Middleware>>,
    pub(crate) endpoint: Rc<dyn Endpoint>,
}

impl Clone for Layer {
    fn clone(&self) -> Self {
        Self {
            middlewares: self
                .middlewares
                .iter()
                .map(|rc_dyn| Rc::from(dyn_clone::clone_box(&**rc_dyn)))
                .collect(),
            endpoint: Rc::from(dyn_clone::clone_box(&*self.endpoint)),
        }
    }
}

impl Layer {
    pub(crate) fn new(endpoint: impl Endpoint) -> Self {
        Layer {
            middlewares: Default::default(),
            endpoint: Rc::new(endpoint),
        }
    }

    pub(crate) fn append(&mut self, m: Rc<impl Middleware>) {
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
