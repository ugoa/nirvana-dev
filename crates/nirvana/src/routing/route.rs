pub mod tower_impl;

use http::Method;
use pin_project_lite::pin_project;

use tower::util::Oneshot;

use crate::prelude::*;

use std::{
    convert::Infallible,
    pin::Pin,
    task::{Context, Poll, ready},
};

use tower::ServiceExt;

use self::tower_impl::{LocalBoxCloneService, MapIntoResponse};

pub struct Route<E = Infallible>(LocalBoxCloneService<Request, Response, E>);

impl<E> Clone for Route<E> {
    #[track_caller]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<E> Route<E> {
    pub fn new<T>(svc: T) -> Self
    where
        T: TowerService<Request, Error = E> + Clone + 'static,
        T::Response: IntoResponse + 'static,
        T::Future: 'static,
    {
        Self(LocalBoxCloneService::new(MapIntoResponse::new(svc)))
    }

    pub fn oneshot_inner(&self, req: Request) -> RouteFuture<E> {
        let method = req.method().clone();
        RouteFuture::new(method, self.0.clone().oneshot(req))
    }

    pub fn oneshot_inner_owned(self, req: Request) -> RouteFuture<E> {
        let method = req.method().clone();
        RouteFuture::new(method, self.0.oneshot(req))
    }
}

pin_project! {
    /// Response future for [`Route`].
    pub struct RouteFuture<E> {
        #[pin]
        inner: Oneshot<LocalBoxCloneService<Request,Response,E> , Request>,
        method: Method,
    }
}

impl<E> RouteFuture<E> {
    fn new(
        method: Method,
        inner: Oneshot<LocalBoxCloneService<Request, Response, E>, Request>,
    ) -> Self {
        Self { inner, method }
    }
}

impl<E> Future for RouteFuture<E> {
    type Output = Result<Response, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        let mut resp = std::task::ready!(this.inner.poll(cx))?;

        Poll::Ready(Ok(resp))
    }
}

pin_project! {
    pub(crate) struct MapIntoResponseFuture<F> {
        #[pin]
        pub inner: F,
    }
}

impl<F, T, E> Future for MapIntoResponseFuture<F>
where
    F: Future<Output = Result<T, E>>,
    T: IntoResponse,
{
    type Output = Result<Response, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let res = ready!(self.project().inner.poll(cx)?);

        Poll::Ready(Ok(res.into_response()))
        // Here every different types of return values from handler turn into Response
    }
}
