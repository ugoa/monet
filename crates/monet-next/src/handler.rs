use crate::{Request, Response};

trait Handler {
    async fn call(
        &self,
        req: &mut Request,
        state: &mut State,
        ctrl: &mut FlowCtrl,
        resp: &mut Response,
    );
}
