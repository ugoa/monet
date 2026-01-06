use crate::{
    HttpRequest, HttpResponse,
    extract::{FromRequest, FromRequestParts},
    response::IntoResponse,
};
use std::pin::Pin;

// X for Extractor
pub trait Handler<'a, X, S>: Clone + Sized + 'a {
    type Future: Future<Output = HttpResponse<'a>> + 'a;

    fn call(self, req: HttpRequest, state: S) -> Self::Future;

    fn to_service(self, state: S) -> HandlerService<'a, Self, X, S> {
        HandlerService::new(self, state)
    }
}

pub trait HandlerWithoutStateExt<'a, T>: Handler<'a, T, ()> {
    /// Convert the handler into a [`Service`] and no state.
    fn into_service(self) -> HandlerService<'a, Self, T, ()>;
}

impl<'a, H, T> HandlerWithoutStateExt<'a, T> for H
where
    H: Handler<'a, T, ()>,
{
    fn into_service(self) -> HandlerService<'a, Self, T, ()> {
        self.to_service(())
    }
}

impl<'a, F, Fut, Res, S> Handler<'a, ((),), S> for F
where
    F: FnOnce() -> Fut + Clone + 'a,
    Fut: Future<Output = Res>,
    Res: IntoResponse,
{
    type Future = Pin<Box<dyn Future<Output = HttpResponse<'a>> + 'a>>;

    fn call(self, _req: HttpRequest, _state: S) -> Self::Future {
        Box::pin(async move { self().await.into_response() })
    }
}

#[allow(non_snake_case, unused_mut)]
impl<'a, F, Fut, S, Res, M, T1, T2, T3> Handler<'a, (M, T1, T2, T3), S> for F
where
    F: FnOnce(T1, T2, T3) -> Fut + Clone + 'a,
    Fut: Future<Output = Res>,
    S: 'a,
    Res: IntoResponse,
    T1: FromRequestParts<S>,
    T2: FromRequestParts<S>,
    T3: FromRequest<S, M>,
{
    type Future = Pin<Box<dyn Future<Output = HttpResponse<'a>> + 'a>>;
    fn call(self, req: HttpRequest, state: S) -> Self::Future {
        let (mut parts, body) = req.into_parts();
        Box::pin(async move {
            let T1 = match T1::from_request_parts(&mut parts, &state).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let T2 = match T2::from_request_parts(&mut parts, &state).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let req = HttpRequest::from_parts(parts, body);
            let T3 = match T3::from_request(req, &state).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            self(T1, T2, T3).await.into_response()
        })
    }
}

// macro_rules! impl_handler {
//     (
//         [$($ty:ident),*], $last:ident
//     ) => {
//         #[allow(non_snake_case, unused_mut)]
//         impl<F, Fut, S, Res, M, $($ty,)* $last> Handler<(M, $($ty,)* $last,), S> for F
//         where
//             F: FnOnce($($ty,)* $last,) -> Fut + Clone +  'static,
//             Fut: Future<Output = Res>,
//             S: 'static,
//             Res: IntoResponse,
//             $( $ty: FromRequestParts<S>, )*
//             $last: FromRequest<S, M>,
//         {
//             type Future = Pin<Box<dyn Future<Output = HttpResponse>>>;
//
//             fn call(self, req: HttpRequest, state: S) -> Self::Future {
//                 let (mut parts, body) = req.into_parts();
//                 Box::pin(async move {
//                     $(
//                         let $ty = match $ty::from_request_parts(&mut parts, &state).await {
//                             Ok(value) => value,
//                             Err(rejection) => return rejection.into_response(),
//                         };
//                     )*
//
//                     let req = HttpRequest::from_parts(parts, body);
//
//                     let $last = match $last::from_request(req, &state).await {
//                         Ok(value) => value,
//                         Err(rejection) => return rejection.into_response(),
//                     };
//
//                     self($($ty,)* $last,).await.into_response()
//                 })
//             }
//         }
//     };
// }
//
// #[rustfmt::skip]
// macro_rules! all_the_tuples {
//     ($name:ident) => {
//         // $name!([], T1);
//         // $name!([T1], T2);
//         // $name!([T1, T2], T3);
//         $name!([T1, T2, T3], T4);
//         $name!([T1, T2, T3, T4], T5);
//         $name!([T1, T2, T3, T4, T5], T6);
//         $name!([T1, T2, T3, T4, T5, T6], T7);
//         $name!([T1, T2, T3, T4, T5, T6, T7], T8);
//         $name!([T1, T2, T3, T4, T5, T6, T7, T8], T9);
//         $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9], T10);
//         $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10], T11);
//         $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11], T12);
//         $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12], T13);
//         $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13], T14);
//         $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14], T15);
//         $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15], T16);
//     };
// }
//
// all_the_tuples!(impl_handler);
//
// #[allow(non_snake_case, unused_mut)]
// impl<F, Fut, S, Res, M, T1> Handler<(M, T1), S> for F
// where
//     F: FnOnce(T1) -> Fut + Clone + 'static,
//     Fut: Future<Output = Res>,
//     S: 'static,
//     Res: IntoResponse,
//     T1: FromRequest<S, M>,
// {
//     type Future = Pin<Box<dyn Future<Output = HttpResponse>>>;
//     fn call(self, req: HttpRequest, state: S) -> Self::Future {
//         let (mut parts, body) = req.into_parts();
//         Box::pin(async move {
//             let req = HttpRequest::from_parts(parts, body);
//             let T1 = match T1::from_request(req, &state).await {
//                 Ok(value) => value,
//                 Err(rejection) => return rejection.into_response(),
//             };
//             self(T1).await.into_response()
//         })
//     }
// }
// #[allow(non_snake_case, unused_mut)]
// impl<F, Fut, S, Res, M, T1, T2> Handler<(M, T1, T2), S> for F
// where
//     F: FnOnce(T1, T2) -> Fut + Clone + 'static,
//     Fut: Future<Output = Res>,
//     S: 'static,
//     Res: IntoResponse,
//     T1: FromRequestParts<S>,
//     T2: FromRequest<S, M>,
// {
//     type Future = Pin<Box<dyn Future<Output = HttpResponse>>>;
//     fn call(self, req: HttpRequest, state: S) -> Self::Future {
//         let (mut parts, body) = req.into_parts();
//         Box::pin(async move {
//             let T1 = match T1::from_request_parts(&mut parts, &state).await {
//                 Ok(value) => value,
//                 Err(rejection) => return rejection.into_response(),
//             };
//             let req = HttpRequest::from_parts(parts, body);
//             let T2 = match T2::from_request(req, &state).await {
//                 Ok(value) => value,
//                 Err(rejection) => return rejection.into_response(),
//             };
//             self(T1, T2).await.into_response()
//         })
//     }
// }

use std::{fmt, marker::PhantomData};

// X represents the handler's arguments which are all extractors
pub struct HandlerService<'a, H, X, S> {
    pub handler: H,
    pub state: S,
    // The handler service is covariant in X, which is similar to
    //     PhantomData<X>,
    // the above is Send, and Sync if and only if X is Sync too.
    // while fn() -> X is always Send and Sync, because function pointer
    // type is always Send and Sync
    pub(crate) _marker: PhantomData<fn() -> X>,
    // Consider to use below when the Send and Sync bound is not necessary
    // pub(crate) _marker: PhantomData<&X>,
    pub(crate) _lt: PhantomData<&'a ()>, // &'a () is covariant in 'a
}

impl<H, X, S> Clone for HandlerService<'_, H, X, S>
where
    H: Clone,
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            state: self.state.clone(),
            _marker: PhantomData,
            _lt: PhantomData,
        }
    }
}

impl<H, X, S> HandlerService<'_, H, X, S> {
    pub(super) fn new(handler: H, state: S) -> Self {
        Self {
            handler,
            state,
            _marker: PhantomData,
            _lt: PhantomData,
        }
    }
    pub fn state(&self) -> &S {
        &self.state
    }
}

impl<H, T, S> fmt::Debug for HandlerService<'_, H, T, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IntoService").finish_non_exhaustive()
    }
}
