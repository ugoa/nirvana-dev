use std::{
    convert::Infallible,
    marker::PhantomData,
    task::{Context, Poll},
};

use futures::future::Map;

use crate::{
    Body, BoxError, HttpBody, HttpRequest, HttpResponse, TowerService,
    extract::{FromRequest, FromRequestParts},
    handler::{Handler, HandlerService},
    opaque_future,
    response::IntoResponse,
};

impl<H, X, S, B> TowerService<HttpRequest<B>> for HandlerService<H, X, S>
where
    H: Handler<X, S> + Clone + 'static,
    B: HttpBody<Data = bytes::Bytes> + 'static,
    B::Error: Into<BoxError>,
    S: Clone,
{
    type Response = HttpResponse;

    type Error = Infallible;

    type Future = IntoServiceFuture<H::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: HttpRequest<B>) -> Self::Future {
        use futures_util::future::FutureExt;
        let req = req.map(Body::new);
        let handler = self.handler.clone();

        let future = Handler::call(handler, req, self.state.clone());

        let future = future.map(Ok as _);

        IntoServiceFuture::new(future)
    }
}

opaque_future! {
    /// The response future for [`IntoService`](super::IntoService).
    pub type IntoServiceFuture<F> =
        Map<
            F,
            fn(HttpResponse) -> Result<HttpResponse, Infallible>,
        >;
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
