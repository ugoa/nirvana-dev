use crate::prelude::*;
use crate::routing::method_router::MethodRouter;
use crate::routing::route_tower_impl::RouteFuture;
use crate::{handler::Handler, routing::route::BoxedIntoRoute};
use matchit::MatchError;
use std::rc::Rc;
use std::{collections::HashMap, convert::Infallible};
use tower::Layer;

#[must_use]
#[derive(Clone)]
pub struct Router<S = ()> {
    pub routes: Vec<Endpoint<S>>,
    pub node: Node,
    pub default_fallback: bool,
    pub catch_all_fallback: Fallback<S>,
}

impl<S> fmt::Debug for Router<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Router")
            .field("routes", &self.routes)
            .field("node", &self.node)
            .field("default_fallback", &self.default_fallback)
            .field("catch_all_fallback", &self.catch_all_fallback)
            .finish()
    }
}

impl<S> Default for Router<S>
where
    S: Clone + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct NotFound;

impl<S> Router<S>
where
    S: Clone + 'static,
{
    pub fn new() -> Self {
        Self {
            routes: Default::default(),
            node: Default::default(),
            default_fallback: true,
            catch_all_fallback: Fallback::Default(Route::new(NotFound)),
        }
    }

    pub fn route(self, path: &str, method_router: MethodRouter<S>) -> Self {
        let mut this = self.clone();

        match (this.process_route(path, method_router)) {
            Ok(x) => x,
            Err(err) => {
                panic!("{err}")
            }
        };
        this
    }

    fn process_route(&mut self, path: &str, method_router: MethodRouter<S>) -> Result<(), String> {
        let mut this = self.clone();
        if let Some(route_id) = this.node.path_to_route_id.get(path) {
            if let Some(Endpoint::MethodRouter(prev_method_router)) = this.routes.get(route_id.0) {
                let service = Endpoint::MethodRouter(
                    prev_method_router
                        .clone()
                        .merge_for_path(Some(path), method_router)
                        .unwrap(),
                );
                this.routes[route_id.0] = service;
            }
        } else {
            let endpoint = Endpoint::MethodRouter(method_router);
            this.new_route(path, endpoint).unwrap();
        }
        Ok(())
    }

    fn new_route(&mut self, path: &str, endpoint: Endpoint<S>) -> Result<(), String> {
        let id = RouteId(self.routes.len());
        self.set_node(path, id)?;
        self.routes.push(endpoint);
        Ok(())
    }

    fn set_node(&mut self, path: &str, id: RouteId) -> Result<(), String> {
        self.node
            .insert(path, id)
            .map_err(|err| format!("Invalid route {path:?}: {err}"))
    }

    pub fn merge<R>(self, other: R) -> Self
    where
        R: Into<Self>,
    {
        let mut this = self.clone();
        let other: Self = other.into();

        let default_fallback = match (this.default_fallback, other.default_fallback) {
            (_, true) => this.default_fallback,
            (true, false) => false,

            (false, false) => {
                panic!("Cannot merge two `Router`s that both have a fallback");
            }
        };

        let catch_all_fallback = this
            .catch_all_fallback
            .clone()
            .merge(other.catch_all_fallback)
            .unwrap_or_else(|| panic!("Cannot merge two `Router`s that both have a fallback"));

        for (id, route) in other.routes.into_iter().enumerate() {
            let route_id = RouteId(id);
            let path = other
                .node
                .route_id_to_path
                .get(&route_id)
                .expect("no path for route id. This is a bug in axum. Please file an issue");

            match route {
                Endpoint::MethodRouter(method_router) => {
                    this.process_route(path, method_router).unwrap()
                }
                Endpoint::Route(service) => this
                    .new_route(path, Endpoint::Route(Route::new(service)))
                    .unwrap(),
            }
        }
        Router {
            routes: this.routes,
            node: this.node,
            default_fallback: default_fallback,
            catch_all_fallback: catch_all_fallback,
        }
    }

    pub fn fallback<H, T>(mut self, handler: H) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
    {
        self.catch_all_fallback =
            Fallback::BoxedHandler(BoxedIntoRoute::from_handler(handler.clone()));
        self
    }

    pub fn layer<L>(mut self, layer: L) -> Self
    where
        L: Layer<Route> + Clone + 'static,
        L::Service: TowerService<Request> + Clone + 'static,
        <L::Service as TowerService<Request>>::Response: IntoResponse + 'static,
        <L::Service as TowerService<Request>>::Error: Into<Infallible> + 'static,
        <L::Service as TowerService<Request>>::Future: 'static,
    {
        self.routes = self
            .routes
            .into_iter()
            .map(|endpoint| endpoint.layer(layer.clone()))
            .collect();

        self.catch_all_fallback = self.catch_all_fallback.map(|route| route.layer(layer));
        self
    }

    pub fn route_layer<L>(mut self, layer: L) -> Self
    where
        L: Layer<Route> + Clone + 'static,
        L::Service: TowerService<Request> + Clone + 'static,
        <L::Service as TowerService<Request>>::Response: IntoResponse + 'static,
        <L::Service as TowerService<Request>>::Error: Into<Infallible> + 'static,
        <L::Service as TowerService<Request>>::Future: 'static,
    {
        self.routes = self
            .routes
            .into_iter()
            .map(|endpoint| endpoint.layer(layer.clone()))
            .collect();
        self
    }

    pub fn with_state<S2>(mut self, state: S) -> Router<S2> {
        let routes = self
            .routes
            .into_iter()
            .map(|endpoint| match endpoint {
                Endpoint::MethodRouter(method_router) => {
                    Endpoint::MethodRouter(method_router.with_state(state.clone()))
                }
                Endpoint::Route(route) => Endpoint::Route(route),
            })
            .collect();

        Router {
            routes,
            node: self.node,
            default_fallback: self.default_fallback,
            catch_all_fallback: self.catch_all_fallback.with_state(state),
        }
    }

    pub(crate) fn call_with_state(&self, req: Request, state: S) -> RouteFuture<Infallible> {
        let (mut parts, body) = req.into_parts();

        println!("{:?}", &self);

        match self.node.at(parts.uri.path()) {
            Ok(matched) => {
                let route_id = matched.value;

                let endpoint = self.routes.get(route_id.0).expect(
                    "It is granted a valid route for id. Please file an issue if it is not",
                );

                let req = Request::from_parts(parts, body);

                match endpoint {
                    Endpoint::MethodRouter(method_router) => {
                        method_router.call_with_state(req, state)
                    }
                    Endpoint::Route(route) => route.clone().call_owned(req),
                }
            }
            Err(MatchError::NotFound) => {
                let req = Request::from_parts(parts, body);
                self.catch_all_fallback.clone().call_with_state(req, state)
            }
        }
    }
}

#[allow(clippy::large_enum_variant)]
pub enum Endpoint<S> {
    MethodRouter(MethodRouter<S>),
    Route(Route),
}

impl<S> fmt::Debug for Endpoint<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MethodRouter(method_router) => {
                f.debug_tuple("MethodRouter").field(method_router).finish()
            }
            Self::Route(route) => f.debug_tuple("Route").field(route).finish(),
        }
    }
}

impl<S> Endpoint<S>
where
    S: Clone + 'static,
{
    pub fn layer<L>(self, layer: L) -> Self
    where
        L: Layer<Route> + Clone + 'static,
        L::Service: TowerService<Request> + Clone + 'static,
        <L::Service as TowerService<Request>>::Response: IntoResponse + 'static,
        <L::Service as TowerService<Request>>::Error: Into<Infallible> + 'static,
        <L::Service as TowerService<Request>>::Future: 'static,
    {
        match self {
            Self::Route(route) => Self::Route(route.layer(layer)),
            Self::MethodRouter(method_router) => Self::MethodRouter(method_router.layer(layer)),
        }
    }
}

impl<S> Clone for Endpoint<S> {
    fn clone(&self) -> Self {
        match self {
            Self::MethodRouter(inner) => Self::MethodRouter(inner.clone()),
            Self::Route(inner) => Self::Route(inner.clone()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RouteId(pub usize);

#[derive(Clone, Default)]
pub struct Node {
    pub inner: matchit::Router<RouteId>,
    pub route_id_to_path: HashMap<RouteId, String>,
    pub path_to_route_id: HashMap<String, RouteId>,
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Node")
            .field("paths", &self.route_id_to_path)
            .finish()
    }
}

impl Node {
    pub fn insert(
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

    pub fn at<'n, 'p>(
        &'n self,
        path: &'p str,
    ) -> Result<matchit::Match<'n, 'p, &'n RouteId>, MatchError> {
        self.inner.at(path)
    }
}

pub(crate) enum Fallback<S, E = Infallible> {
    Default(Route<E>),
    Service(Route<E>),
    BoxedHandler(BoxedIntoRoute<S, E>),
}

impl<S, E> Clone for Fallback<S, E> {
    fn clone(&self) -> Self {
        match self {
            Self::Default(inner) => Self::Default(inner.clone()),
            Self::Service(inner) => Self::Service(inner.clone()),
            Self::BoxedHandler(inner) => Self::BoxedHandler(inner.clone()),
        }
    }
}
impl<S, E> fmt::Debug for Fallback<S, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default(inner) => f.debug_tuple("Default").field(inner).finish(),
            Self::Service(inner) => f.debug_tuple("Service").field(inner).finish(),
            Self::BoxedHandler(_) => f.debug_tuple("BoxedHandler").finish(),
        }
    }
}

impl<S, E> Fallback<S, E>
where
    S: Clone,
{
    pub fn merge(self, other: Self) -> Option<Self> {
        match (self, other) {
            // If either are `Default`, return the opposite one.
            (Self::Default(_), pick) => Some(pick),
            (pick, Self::Default(_)) => Some(pick),
            // Otherwise, return None
            _ => None,
        }
    }

    pub fn map<F, E2>(self, f: F) -> Fallback<S, E2>
    where
        S: 'static,
        E: 'static,
        F: FnOnce(Route<E>) -> Route<E2> + Clone + 'static,
        E2: 'static,
    {
        match self {
            Self::Default(route) => Fallback::Default(f(route)),
            Self::Service(route) => Fallback::Service(f(route)),
            Self::BoxedHandler(handler) => Fallback::BoxedHandler(handler.map(f)),
        }
    }

    pub fn with_state<S2>(self, state: S) -> Fallback<S2, E> {
        match self {
            Self::Default(route) => Fallback::Default(route),
            Self::Service(route) => Fallback::Service(route),
            Self::BoxedHandler(handler) => Fallback::Service(handler.into_route(state)),
        }
    }

    pub fn call_with_state(self, req: Request, state: S) -> RouteFuture<E> {
        match self {
            Self::Default(route) | Self::Service(route) => route.call(req),
            Self::BoxedHandler(handler) => {
                let route = handler.into_route(state);
                route.call(req)
            }
        }
    }
}
