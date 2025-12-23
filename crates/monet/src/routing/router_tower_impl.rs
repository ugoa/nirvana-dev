use crate::{
    Body, BoxError, HttpBody, HttpRequest, HttpResponse, IntoResponse, TowerService,
    routing::{
        route_tower_impl::RouteFuture,
        router::{NotFound, Router},
    },
    serve::{IncomingStream, Listener},
};
use std::{
    convert::Infallible,
    future::ready,
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
        std::future::ready(Ok(self.clone().with_state(())))
    }
}

impl<B> TowerService<HttpRequest<B>> for Router<()>
where
    B: HttpBody<Data = bytes::Bytes> + 'static,
    B::Error: Into<BoxError>,
{
    type Response = HttpResponse;

    type Error = Infallible;

    type Future = RouteFuture<Infallible>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: HttpRequest<B>) -> Self::Future {
        let req = req.map(Body::new);
        self.call_with_state(req, ())
    }
}

impl<B> TowerService<HttpRequest<B>> for NotFound
where
    B: 'static,
{
    type Response = HttpResponse;
    type Error = Infallible;
    type Future = std::future::Ready<Result<HttpResponse, Self::Error>>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: HttpRequest<B>) -> Self::Future {
        ready(Ok(http::StatusCode::NOT_FOUND.into_response()))
    }
}
