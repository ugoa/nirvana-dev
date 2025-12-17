use crate::{
    Body, BoxError, Bytes, HttpBody, HttpRequest, Request, Response, TowerService,
    extract::{FromRequest, FromRequestParts},
    handler::handler_service::HandlerService,
    opaque_future,
    response::IntoResponse,
};
use futures::future::Map;
use std::{
    convert::Infallible,
    fmt,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

pub mod handler_service;
pub mod handler_service_tower;

// X for Extractor
pub trait Handler<X, S>: Clone + Sized + 'static {
    type Future: Future<Output = Response> + 'static;

    fn call(self, req: Request, state: S) -> Self::Future;

    fn with_state(self, state: S) -> HandlerService<Self, X, S> {
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
        self.with_state(())
    }
}

impl<F, Fut, Res, S> Handler<((),), S> for F
where
    F: FnOnce() -> Fut + Clone + 'static,
    Fut: Future<Output = Res>,
    Res: IntoResponse,
{
    type Future = Pin<Box<dyn Future<Output = Response>>>;

    fn call(self, _req: Request, _state: S) -> Self::Future {
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
            $( $ty: FromRequestParts<S>, )*
            $last: FromRequest<S, M>,
        {
            type Future = Pin<Box<dyn Future<Output = Response>>>;

            fn call(self, req: Request, state: S) -> Self::Future {
                let (mut parts, body) = req.into_parts();
                Box::pin(async move {
                    $(
                        let $ty = match $ty::from_request_parts(&mut parts, &state).await {
                            Ok(value) => value,
                            Err(rejection) => return rejection.into_response(),
                        };
                    )*

                    let req = Request::from_parts(parts, body);

                    let $last = match $last::from_request(req, &state).await {
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
        // $name!([], T1);
        // $name!([T1], T2);
        // $name!([T1, T2], T3);
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

#[allow(non_snake_case, unused_mut)]
impl<F, Fut, S, Res, M, T1> Handler<(M, T1), S> for F
where
    F: FnOnce(T1) -> Fut + Clone + 'static,
    Fut: Future<Output = Res>,
    S: 'static,
    Res: IntoResponse,
    T1: FromRequest<S, M>,
{
    type Future = Pin<Box<dyn Future<Output = Response>>>;
    fn call(self, req: Request, state: S) -> Self::Future {
        let (mut parts, body) = req.into_parts();
        Box::pin(async move {
            let req = Request::from_parts(parts, body);
            let T1 = match T1::from_request(req, &state).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            self(T1).await.into_response()
        })
    }
}
#[allow(non_snake_case, unused_mut)]
impl<F, Fut, S, Res, M, T1, T2> Handler<(M, T1, T2), S> for F
where
    F: FnOnce(T1, T2) -> Fut + Clone + 'static,
    Fut: Future<Output = Res>,
    S: 'static,
    Res: IntoResponse,
    T1: FromRequestParts<S>,
    T2: FromRequest<S, M>,
{
    type Future = Pin<Box<dyn Future<Output = Response>>>;
    fn call(self, req: Request, state: S) -> Self::Future {
        let (mut parts, body) = req.into_parts();
        Box::pin(async move {
            let T1 = match T1::from_request_parts(&mut parts, &state).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            let req = Request::from_parts(parts, body);
            let T2 = match T2::from_request(req, &state).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            self(T1, T2).await.into_response()
        })
    }
}
#[allow(non_snake_case, unused_mut)]
impl<F, Fut, S, Res, M, T1, T2, T3> Handler<(M, T1, T2, T3), S> for F
where
    F: FnOnce(T1, T2, T3) -> Fut + Clone + 'static,
    Fut: Future<Output = Res>,
    S: 'static,
    Res: IntoResponse,
    T1: FromRequestParts<S>,
    T2: FromRequestParts<S>,
    T3: FromRequest<S, M>,
{
    type Future = Pin<Box<dyn Future<Output = Response>>>;
    fn call(self, req: Request, state: S) -> Self::Future {
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
            let req = Request::from_parts(parts, body);
            let T3 = match T3::from_request(req, &state).await {
                Ok(value) => value,
                Err(rejection) => return rejection.into_response(),
            };
            self(T1, T2, T3).await.into_response()
        })
    }
}
