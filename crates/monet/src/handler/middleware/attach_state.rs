use std::pin::Pin;

use crate::{Chain, Middleware, Request, Response};

pub struct AttachState<T> {
    value: T,
}

impl<T: Clone + 'static> AttachState<T> {
    pub fn new(value: T) -> Self {
        AttachState { value }
    }
}
impl<T: Clone + 'static> Middleware for AttachState<T> {
    fn transform(
        &self,
        mut req: Request,
        chain: Chain,
    ) -> Pin<Box<dyn Future<Output = Response> + '_>> {
        Box::pin(async move {
            req.extensions.insert(self.value.clone());
            chain.next(req).await
        })
    }
}
