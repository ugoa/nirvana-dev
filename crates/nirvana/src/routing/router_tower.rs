use crate::{
    Body, BoxError, HttpBody, Request, Response, Router, TowerService,
    routing::route_tower::RouteFuture,
    serve::{IncomingStream, Listener},
};
use std::{
    convert::Infallible,
    task::{Context, Poll},
};

impl<L> TowerService<IncomingStream<'_, L>> for Router<()>
where
    L: Listener,
{
    type Response = Self;

    type Error = Infallible;

    type Future = std::future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: IncomingStream<'_, L>) -> Self::Future {
        std::future::ready(Ok(self.with_state(())))
    }
}

impl<B> TowerService<Request<B>> for Router<()>
where
    B: HttpBody<Data = bytes::Bytes> + 'static,
    B::Error: Into<BoxError>,
{
    type Response = Response;

    type Error = Infallible;

    type Future = RouteFuture<Infallible>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        let req = req.map(Body::new);
        self.call_with_state(req, ())
    }
}
