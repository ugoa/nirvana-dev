use crate::{
    Body, BoxError, Bytes, HttpBody, HttpRequest, Request, Response, TowerService,
    extract::{FromRequest, FromRequestParts},
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

impl<H, T, S> fmt::Debug for HandlerService<H, T, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IntoService").finish_non_exhaustive()
    }
}
