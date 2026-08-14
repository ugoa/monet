use std::pin::Pin;

use crate::{Chain, Middleware, Request, Response};

pub struct AttachState<T> {
    pub value: T,
}

impl<T: Clone + 'static> Middleware for AttachState<T> {
    fn transform(
        &self,
        mut req: Request,
        chain: Chain,
    ) -> Pin<Box<dyn Future<Output = Response> + '_>> {
        Box::pin(async move {
            req.add_state(self.value.clone());
            chain.next(req).await
        })
    }
}
