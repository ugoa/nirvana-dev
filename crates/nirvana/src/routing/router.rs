use crate::prelude::*;
use crate::routing::method_router::MethodRouter;
use crate::routing::route_tower::RouteFuture;
use crate::{handler::Handler, routing::route::BoxedIntoRoute};
use matchit::MatchError;
use std::rc::Rc;
use std::{collections::HashMap, convert::Infallible};
use tower::Layer;

#[derive(Clone)]
pub struct SimpleRouter<S = ()> {
    routes: Vec<MethodRouter<S>>,
    node: Node,
}

#[must_use]
pub struct Router<S = ()> {
    inner: Rc<RouterInner<S>>,
}

impl<S> Clone for Router<S> {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
        }
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

impl<S> Router<S>
where
    S: Clone + 'static,
{
    /// Create a new `Router`.
    ///
    /// Unless you add additional routes, this will respond with `404 Not Found` to
    /// all requests.
    pub fn new() -> Self {
        Self {
            inner: Rc::new(todo!()),
        }
    }
}

struct RouterInner<S> {
    path_router: PathRouter<S>,
    default_fallback: bool,
    catch_all_fallback: Fallback<S>,
}

enum Fallback<S, E = Infallible> {
    Default(Route<E>),
    Service(Route<E>),
    BoxedHandler(BoxedIntoRoute<S, E>),
}

pub(super) struct PathRouter<S> {
    routes: Vec<Endpoint<S>>,
    node: Node,
}

impl<S> PathRouter<S>
where
    S: Clone + 'static,
{
    pub(super) fn layer<L>(self, layer: L) -> Self
    where
        L: Layer<Route> + Clone + 'static,
        L::Service: TowerService<Request> + Clone + 'static,
        <L::Service as TowerService<Request>>::Response: IntoResponse + 'static,
        <L::Service as TowerService<Request>>::Error: Into<Infallible> + 'static,
        <L::Service as TowerService<Request>>::Future: 'static,
    {
        let routes = self
            .routes
            .into_iter()
            .map(|endpoint| endpoint.layer(layer.clone()))
            .collect();
        Self {
            routes,
            node: self.node,
        }
    }
}

#[allow(clippy::large_enum_variant)]
enum Endpoint<S> {
    MethodRouter(MethodRouter<S>),
    Route(Route),
}

impl<S> Endpoint<S>
where
    S: Clone + 'static,
{
    fn layer<L>(self, layer: L) -> Self
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

impl<S> SimpleRouter<S>
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
            if let Some(prev_method_router) = self.routes.get(route_id.0) {
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

    pub fn with_state<S2>(&self, state: S) -> SimpleRouter<S2> {
        let method_routers = (0..self.routes.len())
            .map(|i| self.routes[i].clone().with_state(state.clone()))
            .collect();

        let node = self.node.clone();
        SimpleRouter {
            routes: method_routers,
            node: node,
        }
    }

    pub(crate) fn call_with_state(&self, req: Request, state: S) -> RouteFuture<Infallible> {
        let (parts, body) = req.into_parts();

        let matched = self.node.at(parts.uri.path()).unwrap();

        let id = *matched.value;

        let endpoint = self.routes.get(id.0).unwrap();

        let req = Request::from_parts(parts, body);
        endpoint.call_with_state(req, state)
    }
}
