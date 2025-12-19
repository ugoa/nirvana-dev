use std::{collections::HashMap, convert::Infallible, rc::Rc};

use matchit::MatchError;
use tower::Layer;

use crate::{prelude::*, routing::method_router::MethodRouter};

pub(super) struct PathRouter<S> {
    routes: Vec<Endpoint<S>>,
    node: Node,
}

impl<S> PathRouter<S>
where
    S: Clone + 'static,
{
    pub fn route(&mut self, path: &str, method_router: MethodRouter<S>) -> Result<(), String> {
        if let Some(route_id) = self.node.path_to_route_id.get(path) {
            if let Some(Endpoint::MethodRouter(prev_method_router)) = self.routes.get(route_id.0) {
                // merge route
            }
        } else {
            let endpoint = Endpoint::MethodRouter(method_router);
            self.new_route(path, endpoint)?;
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

impl<S> Clone for PathRouter<S> {
    fn clone(&self) -> Self {
        Self {
            routes: self.routes.clone(),
            node: self.node.clone(),
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
