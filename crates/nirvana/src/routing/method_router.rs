use crate::handler::Handler;
use crate::prelude::*;
use crate::routing::route::{BoxedIntoRoute, ErasedIntoRoute, Route};
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
        let endpoint = &MethodEndpoint::BoxedHandler(BoxedIntoRoute::from_handler(handler));
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
    BoxedHandler(BoxedIntoRoute<S, E>),
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
