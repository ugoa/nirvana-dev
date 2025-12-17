use crate::handler::Handler;
use crate::prelude::*;
use crate::routing::route::Route;
use crate::routing::route_tower::RouteFuture;
use http::Method;
use std::convert::Infallible;

pub struct MethodRouter<S = (), E = Infallible> {
    get: MethodEndpoint<S, E>,
    head: MethodEndpoint<S, E>,
    delete: MethodEndpoint<S, E>,
    options: MethodEndpoint<S, E>,
    patch: MethodEndpoint<S, E>,
    post: MethodEndpoint<S, E>,
    put: MethodEndpoint<S, E>,
    trace: MethodEndpoint<S, E>,
    connect: MethodEndpoint<S, E>,
}

impl<S, E> MethodRouter<S, E>
where
    S: Clone,
{
    pub fn new() -> Self {
        Self {
            get: MethodEndpoint::None,
            head: MethodEndpoint::None,
            delete: MethodEndpoint::None,
            options: MethodEndpoint::None,
            patch: MethodEndpoint::None,
            post: MethodEndpoint::None,
            put: MethodEndpoint::None,
            trace: MethodEndpoint::None,
            connect: MethodEndpoint::None,
        }
    }

    pub fn with_state<S2>(self, state: S) -> MethodRouter<S2, E> {
        MethodRouter {
            get: self.get.with_state(&state),
            head: self.head.with_state(&state),
            delete: self.delete.with_state(&state),
            options: self.options.with_state(&state),
            patch: self.patch.with_state(&state),
            post: self.post.with_state(&state),
            put: self.put.with_state(&state),
            trace: self.trace.with_state(&state),
            connect: self.connect.with_state(&state),
        }
    }

    pub fn call_with_state(&self, req: Request, state: S) -> RouteFuture<E> {
        let Self {
            get,
            head,
            delete,
            options,
            patch,
            post,
            put,
            trace,
            connect,
        } = self;

        if *req.method() == Method::GET {
            match get {
                MethodEndpoint::None => todo!(),
                MethodEndpoint::Route(route) => {
                    return route.clone().oneshot_inner_owned(req);
                }
                MethodEndpoint::BoxedHandler(handler) => {
                    let route = handler.clone().into_route(state);
                    return route.oneshot_inner_owned(req);
                }
            }
        } else {
            todo!()
        }
    }
}

impl<S> MethodRouter<S, Infallible>
where
    S: Clone,
{
    pub fn get<H, X>(mut self, handler: H) -> Self
    where
        H: Handler<X, S>,
        X: 'static,
        S: 'static,
    {
        let endpoint = &MethodEndpoint::BoxedHandler(BoxedHandler::from_handler(handler));
        let end = &mut self.get;

        if end.is_some() {
            panic!("Overlapping method route. Cannot add two method routes that both handle `GET`")
        }
        *end = endpoint.clone();
        self
    }
}

impl<S, E> Clone for MethodRouter<S, E> {
    fn clone(&self) -> Self {
        Self {
            get: self.get.clone(),
            head: self.head.clone(),
            delete: self.delete.clone(),
            options: self.options.clone(),
            patch: self.patch.clone(),
            post: self.post.clone(),
            put: self.put.clone(),
            trace: self.trace.clone(),
            connect: self.connect.clone(),
        }
    }
}

pub fn get<H, X, S>(handler: H) -> MethodRouter<S, Infallible>
where
    H: Handler<X, S>,
    X: 'static,
    S: Clone + 'static,
{
    MethodRouter::new().get(handler)
}

enum MethodEndpoint<S, E> {
    None,
    Route(Route<E>),
    BoxedHandler(BoxedHandler<S, E>),
}

impl<S, E> MethodEndpoint<S, E>
where
    S: Clone,
{
    fn is_some(&self) -> bool {
        matches!(self, Self::Route(_) | Self::BoxedHandler(_))
    }

    fn with_state<S2>(self, state: &S) -> MethodEndpoint<S2, E> {
        match self {
            Self::None => MethodEndpoint::None,
            Self::Route(route) => MethodEndpoint::Route(route),
            Self::BoxedHandler(handler) => MethodEndpoint::Route(handler.into_route(state.clone())),
        }
    }
}

impl<S, E> Clone for MethodEndpoint<S, E> {
    fn clone(&self) -> Self {
        match self {
            Self::None => Self::None,
            Self::Route(inner) => Self::Route(inner.clone()),
            Self::BoxedHandler(inner) => Self::BoxedHandler(inner.clone()),
        }
    }
}

pub(crate) struct BoxedHandler<S, E>(Box<dyn ErasedHandlerIntoRoute<S, E>>);

pub(crate) trait ErasedHandlerIntoRoute<S, E> {
    fn clone_box(&self) -> Box<dyn ErasedHandlerIntoRoute<S, E>>;

    fn into_route(self: Box<Self>, state: S) -> Route<E>;
}

impl<S> BoxedHandler<S, Infallible>
where
    S: Clone + 'static,
{
    pub fn from_handler<H, X>(handler: H) -> Self
    where
        H: Handler<X, S>,
        X: 'static,
    {
        Self(Box::new(ErasedHandler {
            handler,
            into_route_fn: |handler, state| Route::new(Handler::with_state(handler, state)),
        }))
    }
}

impl<S, E> BoxedHandler<S, E> {
    pub(crate) fn into_route(self, state: S) -> Route<E> {
        self.0.into_route(state)
    }
}

impl<S, E> Clone for BoxedHandler<S, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

/// This struct stores 2 function pointers:
/// 1. The handler function itself
/// 2. A function that turns handler w/ state into a Route
pub struct ErasedHandler<H, S> {
    pub handler: H,
    pub into_route_fn: fn(H, S) -> Route,
}

impl<H, S> ErasedHandlerIntoRoute<S, Infallible> for ErasedHandler<H, S>
where
    H: Clone + 'static,
    S: 'static,
{
    fn clone_box(&self) -> Box<dyn ErasedHandlerIntoRoute<S, Infallible>> {
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

pub(crate) struct Map<S, E, E2> {
    pub(crate) inner: Box<dyn ErasedHandlerIntoRoute<S, E>>,
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

impl<S, E, E2> ErasedHandlerIntoRoute<S, E2> for Map<S, E, E2>
where
    S: 'static,
    E: 'static,
    E2: 'static,
{
    fn clone_box(&self) -> Box<dyn ErasedHandlerIntoRoute<S, E2>> {
        Box::new(Self {
            inner: self.inner.clone_box(),
            layer: self.layer.clone_box(),
        })
    }

    fn into_route(self: Box<Self>, state: S) -> Route<E2> {
        (self.layer)(self.inner.into_route(state))
    }
}
