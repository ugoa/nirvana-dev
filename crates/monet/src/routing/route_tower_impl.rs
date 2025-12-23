use crate::prelude::*;
use http::Method;
use pin_project_lite::pin_project;
use std::{
    pin::Pin,
    task::{Context, Poll, ready},
};
use tower::util::{Oneshot, ServiceExt};

/// A local boxed [`Service`] trait object with `Clone`. Same with UnsyncBoxService
/// Ref: https://github.com/tower-rs/tower/blob/tower-0.5.2/tower/src/util/boxed/unsync.rs#L12

pub struct LocalBoxCloneService<T, U, E>(
    Box<
        dyn ClonableService<
                T,
                Response = U,
                Error = E,
                Future = Pin<Box<dyn Future<Output = Result<U, E>>>>,
            >,
    >,
);

impl<T, U, E> LocalBoxCloneService<T, U, E> {
    /// Create a new `BoxCloneSyncService`.
    pub fn new<S>(inner: S) -> Self
    where
        S: TowerService<T, Response = U, Error = E> + Clone + 'static,
        S::Future: 'static,
    {
        let inner = inner.map_future(|f| Box::pin(f) as _);
        LocalBoxCloneService(Box::new(inner))
    }
}

impl<T, U, E> Clone for LocalBoxCloneService<T, U, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

impl<T, U, E> TowerService<T> for LocalBoxCloneService<T, U, E> {
    type Response = U;

    type Error = E;

    type Future = Pin<Box<dyn Future<Output = Result<U, E>>>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready(cx)
    }

    fn call(&mut self, req: T) -> Self::Future {
        self.0.call(req)
    }
}

trait ClonableService<S>: TowerService<S> {
    fn clone_box(
        &self,
    ) -> Box<
        dyn ClonableService<
                S,
                Response = Self::Response,
                Error = Self::Error,
                Future = Self::Future,
            >,
    >;
}

impl<S, T> ClonableService<S> for T
where
    T: TowerService<S> + Clone + 'static,
{
    fn clone_box(
        &self,
    ) -> Box<dyn ClonableService<S, Response = T::Response, Error = T::Error, Future = T::Future>>
    {
        Box::new(self.clone())
    }
}

#[derive(Clone)]
pub(crate) struct MapIntoResponse<S> {
    pub inner: S,
}

impl<S> MapIntoResponse<S> {
    pub(crate) fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<B, S> TowerService<http::Request<B>> for MapIntoResponse<S>
where
    S: TowerService<http::Request<B>>,
    S::Response: IntoResponse,
{
    type Response = HttpResponse;
    type Error = S::Error;
    type Future = MapIntoResponseFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<B>) -> Self::Future {
        MapIntoResponseFuture {
            inner: self.inner.call(req),
        }
    }
}

pin_project! {
    /// Response future for [`Route`].
    pub struct RouteFuture<E> {
        #[pin]
        inner: Oneshot<LocalBoxCloneService<HttpRequest,HttpResponse,E> , HttpRequest>,
        method: Method,
    }
}

impl<E> RouteFuture<E> {
    pub fn new(
        method: Method,
        inner: Oneshot<LocalBoxCloneService<HttpRequest, HttpResponse, E>, HttpRequest>,
    ) -> Self {
        Self { inner, method }
    }
}

impl<E> Future for RouteFuture<E> {
    type Output = Result<HttpResponse, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        let resp = std::task::ready!(this.inner.poll(cx))?;

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
    type Output = Result<HttpResponse, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let res = ready!(self.project().inner.poll(cx)?);

        Poll::Ready(Ok(res.into_response()))
        // Here every different types of return values from handler turn into Response
    }
}
