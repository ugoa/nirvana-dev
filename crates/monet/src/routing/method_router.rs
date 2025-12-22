use crate::handler::Handler;
use crate::prelude::*;
use crate::routing::route::{BoxedIntoRoute, ErasedIntoRoute, Route};
use crate::routing::route_tower_impl::RouteFuture;
use crate::routing::router::Fallback;
use http::{Method, StatusCode};
use std::convert::Infallible;
use tower::{Layer, service_fn};

pub fn get<H, X, S>(handler: H) -> MethodRouter<S, Infallible>
where
    H: Handler<X, S>,
    X: 'static,
    S: Clone + 'static,
{
    MethodRouter::new().get(handler)
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
    fallback: Fallback<S, E>,
}

impl<S, E> fmt::Debug for MethodRouter<S, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MethodRouter")
            .field("get", &self.get)
            .field("head", &self.head)
            .field("delete", &self.delete)
            .field("options", &self.options)
            .field("patch", &self.patch)
            .field("post", &self.post)
            .field("put", &self.put)
            .field("trace", &self.trace)
            .field("connect", &self.connect)
            .field("fallback", &self.fallback)
            .finish()
    }
}

impl<S, E> MethodRouter<S, E>
where
    S: Clone,
{
    pub fn new() -> Self {
        let fallback = Route::new(service_fn(|_: Request| async {
            Ok(StatusCode::METHOD_NOT_ALLOWED)
        }));
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
            fallback: Fallback::Default(fallback),
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
            fallback: self.fallback.with_state(state),
        }
    }

    pub fn call_with_state(&self, req: Request, state: S) -> RouteFuture<E> {
        let call_branches = [
            (Method::HEAD, &self.head),
            (Method::HEAD, &self.get),
            (Method::GET, &self.get),
            (Method::POST, &self.post),
            (Method::PATCH, &self.patch),
            (Method::PUT, &self.put),
            (Method::DELETE, &self.delete),
            (Method::TRACE, &self.trace),
            (Method::CONNECT, &self.connect),
        ];

        for (method, endpoint) in call_branches {
            if *req.method() == method {
                match endpoint {
                    MethodEndpoint::Route(route) => {
                        return route.clone().call(req);
                    }
                    MethodEndpoint::BoxedHandler(handler) => {
                        let route = handler.clone().into_route(state);
                        return route.call(req);
                    }
                    MethodEndpoint::None => {}
                }
            }
        }

        // If reached here, it means there is no endpoint found for current request,
        // we use fallback to such case.
        self.fallback.clone().call_with_state(req, state)

        // todo add allow_header
    }

    pub fn layer<L, E2>(self, layer: L) -> MethodRouter<S, E2>
    where
        L: Layer<Route<E>> + Clone + 'static,
        L::Service: TowerService<Request> + Clone + 'static,
        <L::Service as TowerService<Request>>::Response: IntoResponse + 'static,
        <L::Service as TowerService<Request>>::Error: Into<E2> + 'static,
        <L::Service as TowerService<Request>>::Future: 'static,
        E: 'static,
        S: 'static,
        E2: 'static,
    {
        let layer_fn = move |route: Route<E>| route.layer(layer.clone());

        MethodRouter {
            get: self.get.map(layer_fn.clone()),
            head: self.head.map(layer_fn.clone()),
            delete: self.delete.map(layer_fn.clone()),
            options: self.options.map(layer_fn.clone()),
            patch: self.patch.map(layer_fn.clone()),
            post: self.post.map(layer_fn.clone()),
            put: self.put.map(layer_fn.clone()),
            trace: self.trace.map(layer_fn.clone()),
            connect: self.connect.map(layer_fn.clone()),
            fallback: self.fallback.map(layer_fn),
        }
    }

    pub(crate) fn merge_for_path(
        mut self,
        path: Option<&str>,
        other: Self,
    ) -> Result<Self, String> {
        // written using inner functions to generate less IR
        fn merge_inner<S, E>(
            path: Option<&str>,
            name: &str,
            first: MethodEndpoint<S, E>,
            second: MethodEndpoint<S, E>,
        ) -> Result<MethodEndpoint<S, E>, String> {
            match (first, second) {
                (MethodEndpoint::None, MethodEndpoint::None) => Ok(MethodEndpoint::None),
                (pick, MethodEndpoint::None) => Ok(pick),
                (MethodEndpoint::None, pick) => Ok(pick),
                _ => {
                    let error_message = if path.is_some() {
                        "Overlapping method route. Handler for `{name} {path}` already exists"
                    } else {
                        "Overlapping method route. Cannot merge two method routes that both define `{name}`"
                    };
                    Err(format!("error_message").into())
                }
            }
        }

        self.get = merge_inner(path, "GET", self.get, other.get)?;
        self.head = merge_inner(path, "HEAD", self.head, other.head)?;
        self.delete = merge_inner(path, "DELETE", self.delete, other.delete)?;
        self.options = merge_inner(path, "OPTIONS", self.options, other.options)?;
        self.patch = merge_inner(path, "PATCH", self.patch, other.patch)?;
        self.post = merge_inner(path, "POST", self.post, other.post)?;
        self.put = merge_inner(path, "PUT", self.put, other.put)?;
        self.trace = merge_inner(path, "TRACE", self.trace, other.trace)?;
        self.connect = merge_inner(path, "CONNECT", self.connect, other.connect)?;

        self.fallback = self
            .fallback
            .merge(other.fallback)
            .ok_or("Cannot merge two `MethodRouter`s that both have a fallback")?;

        Ok(self)
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
            fallback: self.fallback.clone(),
        }
    }
}

enum MethodEndpoint<S, E> {
    None,
    Route(Route<E>),
    BoxedHandler(BoxedIntoRoute<S, E>),
}

impl<S, E> fmt::Debug for MethodEndpoint<S, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => f.debug_tuple("None").finish(),
            Self::Route(inner) => inner.fmt(f),
            Self::BoxedHandler(_) => f.debug_tuple("BoxedHandler").finish(),
        }
    }
}

impl<S, E> MethodEndpoint<S, E>
where
    S: Clone,
{
    fn is_some(&self) -> bool {
        matches!(self, Self::Route(_) | Self::BoxedHandler(_))
    }

    fn map<F, E2>(self, f: F) -> MethodEndpoint<S, E2>
    where
        S: 'static,
        E: 'static,
        F: FnOnce(Route<E>) -> Route<E2> + Clone + 'static,
        E2: 'static,
    {
        match self {
            Self::None => MethodEndpoint::None,
            Self::Route(route) => MethodEndpoint::Route(f(route)),
            Self::BoxedHandler(handler) => MethodEndpoint::BoxedHandler(handler.map(f)),
        }
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
