use super::route_tower::{LocalBoxCloneService, MapIntoResponse, RouteFuture};
use crate::{handler::Handler, prelude::*};
use std::convert::Infallible;
use tower::{Layer, ServiceExt, util::MapErrLayer};

pub struct Route<E = Infallible>(LocalBoxCloneService<Request, Response, E>);

impl<E> Route<E> {
    pub fn new<T>(svc: T) -> Self
    where
        T: TowerService<Request, Error = E> + Clone + 'static,
        T::Response: IntoResponse + 'static,
        T::Future: 'static,
    {
        Self(LocalBoxCloneService::new(MapIntoResponse::new(svc)))
    }

    /// Variant of [`Route::call`] that takes ownership of the route to avoid cloning.
    pub(crate) fn call_owned(self, req: Request<Body>) -> RouteFuture<E> {
        let req = req.map(Body::new);
        self.oneshot_inner_owned(req)
    }

    pub fn oneshot_inner(&self, req: Request) -> RouteFuture<E> {
        let method = req.method().clone();
        RouteFuture::new(method, self.0.clone().oneshot(req))
    }

    pub fn oneshot_inner_owned(self, req: Request) -> RouteFuture<E> {
        let method = req.method().clone();
        RouteFuture::new(method, self.0.oneshot(req))
    }

    pub fn layer<L, E2>(self, layer: L) -> Route<E2>
    where
        L: Layer<Self> + 'static,
        L::Service: TowerService<Request> + Clone + 'static,
        <L::Service as TowerService<Request>>::Response: IntoResponse + 'static,
        <L::Service as TowerService<Request>>::Error: Into<E2> + 'static,
        <L::Service as TowerService<Request>>::Future: 'static,
        E2: 'static,
    {
        let layer = (MapErrLayer::new(Into::into), layer);

        Route::new(layer.layer(self))
    }
}

impl<E> Clone for Route<E> {
    #[track_caller]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<E> fmt::Debug for Route<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Route").finish()
    }
}

pub(crate) struct BoxedIntoRoute<S, E>(pub Box<dyn ErasedIntoRoute<S, E>>);

pub(crate) trait ErasedIntoRoute<S, E> {
    fn clone_box(&self) -> Box<dyn ErasedIntoRoute<S, E>>;

    fn into_route(self: Box<Self>, state: S) -> Route<E>;
}

impl<S, E> Clone for BoxedIntoRoute<S, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

///  Transfer Layer Map to Route
pub(crate) struct Map<S, E, E2> {
    pub(crate) inner: Box<dyn ErasedIntoRoute<S, E>>,
    pub(crate) layer: Box<dyn LayerFn<E, E2>>,
}

pub(crate) trait LayerFn<E, E2>: FnOnce(Route<E>) -> Route<E2> {
    fn clone_box(&self) -> Box<dyn LayerFn<E, E2>>;
}

impl<F, E, E2> LayerFn<E, E2> for F
where
    F: FnOnce(Route<E>) -> Route<E2> + Clone + 'static,
{
    fn clone_box(&self) -> Box<dyn LayerFn<E, E2>> {
        Box::new(self.clone())
    }
}

impl<S, E, E2> ErasedIntoRoute<S, E2> for Map<S, E, E2>
where
    S: 'static,
    E: 'static,
    E2: 'static,
{
    fn clone_box(&self) -> Box<dyn ErasedIntoRoute<S, E2>> {
        Box::new(Self {
            inner: self.inner.clone_box(),
            layer: self.layer.clone_box(),
        })
    }

    fn into_route(self: Box<Self>, state: S) -> Route<E2> {
        (self.layer)(self.inner.into_route(state))
    }
}

impl<S, E> BoxedIntoRoute<S, E> {
    pub(crate) fn map<F, E2>(self, f: F) -> BoxedIntoRoute<S, E2>
    where
        S: 'static,
        E: 'static,
        F: FnOnce(Route<E>) -> Route<E2> + Clone + 'static,
        E2: 'static,
    {
        BoxedIntoRoute(Box::new(Map {
            inner: self.0,
            layer: Box::new(f),
        }))
    }

    pub(crate) fn into_route(self, state: S) -> Route<E> {
        self.0.into_route(state)
    }
}

///  Transfer handler to Route
impl<S> BoxedIntoRoute<S, Infallible>
where
    S: Clone + 'static,
{
    pub fn from_handler<H, X>(handler: H) -> Self
    where
        H: Handler<X, S>,
        X: 'static,
    {
        Self(Box::new(ErasedHandler {
            handler: handler,
            into_route_fn: |handler, state| Route::new(Handler::with_state(handler, state)),
        }))
    }
}

/// This struct stores 2 function pointers:
/// 1. The handler function itself
/// 2. A function that turns handler w/ state into a Route
pub struct ErasedHandler<H, S> {
    pub handler: H,
    pub into_route_fn: fn(H, S) -> Route,
}

impl<H, S> ErasedIntoRoute<S, Infallible> for ErasedHandler<H, S>
where
    H: Clone + 'static,
    S: 'static,
{
    fn clone_box(&self) -> Box<dyn ErasedIntoRoute<S, Infallible>> {
        Box::new(self.clone())
    }

    fn into_route(self: Box<Self>, state: S) -> Route<Infallible> {
        (self.into_route_fn)(self.handler, state)
    }
}

impl<H, S> Clone for ErasedHandler<H, S>
where
    H: Clone,
{
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            into_route_fn: self.into_route_fn,
        }
    }
}
