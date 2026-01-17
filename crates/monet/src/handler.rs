use crate::{
    HttpRequest, HttpResponse,
    extract::{FromRequest, FromRequestParts},
    response::IntoResponse,
};
use std::pin::Pin;

// X for Extractor
pub trait Handler<X>: Clone + Sized + 'static {
    type Future: Future<Output = HttpResponse> + 'static;

    fn call(self, req: HttpRequest) -> Self::Future;

    fn to_service(self) -> HandlerService<Self, X> {
        HandlerService::new(self)
    }
}

impl<F, Fut, Res> Handler<((),)> for F
where
    F: FnOnce() -> Fut + Clone + 'static,
    Fut: Future<Output = Res>,
    Res: IntoResponse,
{
    type Future = Pin<Box<dyn Future<Output = HttpResponse>>>;

    fn call(self, _req: HttpRequest) -> Self::Future {
        Box::pin(async move { self().await.into_response() })
    }
}

macro_rules! impl_handler {
    (
        [$($ty:ident),*], $last:ident
    ) => {
        #[allow(non_snake_case, unused_mut)]
        impl<F, Fut, Res, M, $($ty,)* $last> Handler<(M, $($ty,)* $last,)> for F
        where
            F: FnOnce($($ty,)* $last,) -> Fut + Clone +  'static,
            Fut: Future<Output = Res>,
            Res: IntoResponse,
            $( $ty: FromRequestParts, )*
            $last: FromRequest<M>,
        {
            type Future = Pin<Box<dyn Future<Output = HttpResponse>>>;

            fn call(self, req: HttpRequest) -> Self::Future {
                let (mut parts, body) = req.into_parts();
                Box::pin(async move {
                    $(
                        let $ty = match $ty::from_request_parts(&mut parts).await {
                            Ok(value) => value,
                            Err(rejection) => return rejection.into_response(),
                        };
                    )*

                    let req = HttpRequest::from_parts(parts, body);

                    let $last = match $last::from_request(req).await {
                        Ok(value) => value,
                        Err(rejection) => return rejection.into_response(),
                    };

                    self($($ty,)* $last,).await.into_response()
                })
            }
        }
    };
}

#[rustfmt::skip]
macro_rules! all_the_tuples {
    ($name:ident) => {
        $name!([], T1);
        $name!([T1], T2);
        $name!([T1, T2], T3);
        $name!([T1, T2, T3], T4);
        $name!([T1, T2, T3, T4], T5);
        $name!([T1, T2, T3, T4, T5], T6);
        $name!([T1, T2, T3, T4, T5, T6], T7);
        $name!([T1, T2, T3, T4, T5, T6, T7], T8);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8], T9);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9], T10);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10], T11);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11], T12);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12], T13);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13], T14);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14], T15);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15], T16);
    };
}

all_the_tuples!(impl_handler);

use std::{fmt, marker::PhantomData};

pub struct HandlerService<H, X> {
    pub handler: H,
    pub(crate) _marker: PhantomData<fn() -> X>,
}

impl<H, X> Clone for HandlerService<H, X>
where
    H: Clone,
{
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            _marker: PhantomData,
        }
    }
}

impl<H, X> HandlerService<H, X> {
    pub(super) fn new(handler: H) -> Self {
        Self {
            handler,
            _marker: PhantomData,
        }
    }
}

impl<H, T> fmt::Debug for HandlerService<H, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IntoService").finish_non_exhaustive()
    }
}
