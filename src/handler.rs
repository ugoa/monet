use crate::{Request, Response};

pub(crate) trait Handler {
    async fn call(&self, req: &mut Request, resp: &mut Response);
}
