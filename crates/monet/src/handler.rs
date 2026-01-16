use crate::{
    HttpRequest, HttpResponse,
    extract::{FromRequest, FromRequestParts},
    response::IntoResponse,
};
use std::pin::Pin;

// X for Extractor
pub trait Handler<X, S>: Clone + Sized + 'static {
    type Future: Future<Output = HttpResponse> + 'static;

    fn call(self, req: HttpRequest, state: S) -> Self::Future;

    fn to_service(self, state: S) -> HandlerService<Self, X, S> {
        HandlerService::new(self, state)
    }
}

pub trait HandlerWithoutStateExt<T>: Handler<T, ()> {
    /// Convert the handler into a [`Service`] and no state.
    fn into_service(self) -> HandlerService<Self, T, ()>;
}

impl<H, T> HandlerWithoutStateExt<T> for H
where
    H: Handler<T, ()>,
{
    fn into_service(self) -> HandlerService<Self, T, ()> {
        self.to_service(())
    }
}

impl<F, Fut, Res, S> Handler<((),), S> for F
where
    F: FnOnce() -> Fut + Clone + 'static,
    Fut: Future<Output = Res>,
    Res: IntoResponse,
{
    type Future = Pin<Box<dyn Future<Output = HttpResponse>>>;

    fn call(self, _req: HttpRequest, _state: S) -> Self::Future {
        Box::pin(async move { self().await.into_response() })
    }
}

macro_rules! impl_handler {
    (
        [$($ty:ident),*], $last:ident
    ) => {
        #[allow(non_snake_case, unused_mut)]
        impl<F, Fut, S, Res, M, $($ty,)* $last> Handler<(M, $($ty,)* $last,), S> for F
        where
            F: FnOnce($($ty,)* $last,) -> Fut + Clone +  'static,
            Fut: Future<Output = Res>,
            S: 'static,
            Res: IntoResponse,
            $( $ty: FromRequestParts, )*
            $last: FromRequest<M>,
        {
            type Future = Pin<Box<dyn Future<Output = HttpResponse>>>;

            fn call(self, req: HttpRequest, state: S) -> Self::Future {
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

pub struct HandlerService<H, X, S> {
    pub handler: H,
    pub state: S,
    pub(crate) _marker: PhantomData<fn() -> X>,
}

impl<H, X, S> Clone for HandlerService<H, X, S>
where
    H: Clone,
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            state: self.state.clone(),
            _marker: PhantomData,
        }
    }
}

impl<H, X, S> HandlerService<H, X, S> {
    pub(super) fn new(handler: H, state: S) -> Self {
        Self {
            handler,
            state,
            _marker: PhantomData,
        }
    }
    pub fn state(&self) -> &S {
        &self.state
    }
}

impl<H, T, S> fmt::Debug for HandlerService<H, T, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IntoService").finish_non_exhaustive()
    }
}
