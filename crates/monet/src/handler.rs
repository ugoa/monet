pub mod endpoint;
pub mod middleware;

use std::{cell::RefCell, pin::Pin, rc::Rc};

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
pub struct Layers(Rc<RefCell<InnerLayers>>);

impl Layers {
    pub(crate) fn new(endpoint: impl Endpoint) -> Self {
        Layers(Rc::new(RefCell::new(InnerLayers {
            middlewares: Default::default(),
            endpoint: Rc::new(endpoint),
        })))
    }

    pub(crate) fn append(&mut self, m: Rc<impl Middleware>) {
        self.0.borrow_mut().middlewares.push(m.clone());
    }
}

#[derive(Debug, Clone)]
pub struct InnerLayers {
    pub middlewares: Vec<Rc<dyn Middleware>>,
    pub endpoint: Rc<dyn Endpoint>,
}

#[derive(Debug, Clone)]
pub struct Chain {
    handlers: Rc<RefCell<InnerLayers>>,
    cursor: isize,
}

impl Chain {
    pub fn from_layers(layers: &Layers) -> Self {
        Self {
            handlers: Rc::clone(&layers.0),
            cursor: layers.0.borrow().middlewares.len() as isize,
        }
    }

    pub async fn next(&self, req: Request) -> Response {
        self.cursor -= 1;

        if self.cursor >= 0 {
            let mw = self
                .handlers
                .borrow()
                .middlewares
                .get(self.cursor as usize)
                .expect("no out-of-bound error");
            mw.transform(req, self).await
        } else {
            self.handlers.borrow().endpoint.call(req).await
        }
    }
}
