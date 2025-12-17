pub mod route;

use futures_util::future::Map;

use matchit::MatchError;
use std::{
    collections::HashMap,
    convert::Infallible,
    fmt,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll, ready},
};
use tower::ServiceExt;

use crate::{Body, extract::FromRequest, routing::route::Route};
use crate::{HttpRequest, handler::Handler};
use crate::{opaque_future, routing::route::tower_impl::RouteFuture};
use http::Method;
use pin_project_lite::pin_project;
use tower::util::Oneshot;

use crate::{BoxError, HttpBody, Request, Response, TowerService, response::IntoResponse};

#[derive(Clone)]
pub struct Router<S = ()> {
    routes: Vec<MethodRouter<S>>,
    node: Node,
}

use crate::serve::{IncomingStream, Listener};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RouteId(usize);

#[derive(Clone, Default)]
struct Node {
    inner: matchit::Router<RouteId>,
    route_id_to_path: HashMap<RouteId, String>,
    path_to_route_id: HashMap<String, RouteId>,
}

impl Node {
    fn insert(
        &mut self,
        path: impl Into<String>,
        val: RouteId,
    ) -> Result<(), matchit::InsertError> {
        let path = path.into();

        self.inner.insert(&path, val)?;

        self.route_id_to_path.insert(val, path.clone());
        self.path_to_route_id.insert(path, val);

        Ok(())
    }

    fn at<'n, 'p>(
        &'n self,
        path: &'p str,
    ) -> Result<matchit::Match<'n, 'p, &'n RouteId>, MatchError> {
        self.inner.at(path)
    }
}

impl<S> Router<S>
where
    S: Clone + 'static,
{
    pub fn new() -> Self {
        Self {
            routes: Default::default(),
            node: Default::default(),
        }
    }

    pub fn route(mut self, path: &str, method_router: MethodRouter<S>) -> Self {
        if let Some(route_id) = self.node.path_to_route_id.get(path) {
            if let Some(mut prev_method_router) = self.routes.get(route_id.0) {
                // merge to existing router
                todo!()
            }
        } else {
            let new_route_id = RouteId(self.routes.len());
            self.node.insert(path, new_route_id);
            self.routes.push(method_router);
        }

        self
    }

    pub fn with_state<S2>(&self, state: S) -> Router<S2> {
        let method_routers = (0..self.routes.len())
            .map(|i| self.routes[i].clone().with_state(state.clone()))
            .collect();

        let node = self.node.clone();
        Router {
            routes: method_routers,
            node: node,
        }
    }

    pub(crate) fn call_with_state(&self, req: Request, state: S) -> RouteFuture<Infallible> {
        let (mut parts, body) = req.into_parts();

        let matched = self.node.at(parts.uri.path()).unwrap();

        let id = *matched.value;

        let endpoint = self.routes.get(id.0).unwrap();

        let req = Request::from_parts(parts, body);
        endpoint.call_with_state(req, state)
    }
}

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

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct MethodFilter(u16);

impl MethodFilter {
    pub const CONNECT: Self = Self::from_bits(0b0_0000_0001);

    pub const DELETE: Self = Self::from_bits(0b0_0000_0010);

    pub const GET: Self = Self::from_bits(0b0_0000_0100);

    pub const HEAD: Self = Self::from_bits(0b0_0000_1000);

    pub const OPTIONS: Self = Self::from_bits(0b0_0001_0000);

    pub const PATCH: Self = Self::from_bits(0b0_0010_0000);

    pub const POST: Self = Self::from_bits(0b0_0100_0000);

    pub const PUT: Self = Self::from_bits(0b0_1000_0000);

    pub const TRACE: Self = Self::from_bits(0b1_0000_0000);

    const fn bits(self) -> u16 {
        let bits = self;
        bits.0
    }

    const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }

    pub(crate) const fn contains(self, other: Self) -> bool {
        self.bits() & other.bits() == other.bits()
    }

    /// Performs the OR operation between the [`MethodFilter`] in `self` with `other`.
    #[must_use]
    pub const fn or(self, other: Self) -> Self {
        Self(self.0 | other.0)
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

pub(crate) struct BoxedIntoRoute<S, E>(Box<dyn ErasedIntoRoute<S, E>>);

impl<S> BoxedIntoRoute<S, Infallible>
where
    S: Clone + 'static,
{
    pub fn from_handler<H, X>(handler: H) -> Self
    where
        H: Handler<X, S>,
        X: 'static,
    {
        Self(Box::new(MakeErasedHandler {
            handler,
            into_route_fn: |handler, state| Route::new(Handler::with_state(handler, state)),
        }))
    }
}

impl<S, E> BoxedIntoRoute<S, E> {
    pub(crate) fn into_route(self, state: S) -> Route<E> {
        self.0.into_route(state)
    }
}

impl<S, E> Clone for BoxedIntoRoute<S, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

/// This struct stores 2 function pointers:
/// 1. The handler function itself
/// 2. A function that turns handler w/ state into a Route
pub struct MakeErasedHandler<H, S> {
    pub handler: H,
    pub into_route_fn: fn(H, S) -> Route,
}

pub(crate) trait ErasedIntoRoute<S, E> {
    fn clone_box(&self) -> Box<dyn ErasedIntoRoute<S, E>>;

    fn into_route(self: Box<Self>, state: S) -> Route<E>;

    #[allow(dead_code)]
    fn call_with_state(self: Box<Self>, request: Request, state: S) -> RouteFuture<E>;
}

impl<H, S> ErasedIntoRoute<S, Infallible> for MakeErasedHandler<H, S>
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

    fn call_with_state(self: Box<Self>, request: Request, state: S) -> RouteFuture<Infallible> {
        self.into_route(state).call(request)
    }
}

impl<H, S> Clone for MakeErasedHandler<H, S>
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
