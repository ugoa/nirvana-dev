use crate::prelude::*;
use crate::routing::method_router::MethodRouter;
use crate::routing::path_router::{Node, PathRouter, RouteId};
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
    pub fn new() -> Self {
        Self {
            inner: Rc::new(todo!()),
        }
    }

    pub fn route(self, path: &str, method_router: MethodRouter<S>) -> Self {
        let mut this = self.into_inner();
        match (this.path_router.route(path, method_router)) {
            Ok(x) => x,
            Err(err) => panic!("{err}"),
        };
        Router {
            inner: Rc::new(this),
        }
    }

    fn into_inner(self) -> RouterInner<S> {
        match Rc::try_unwrap(self.inner) {
            Ok(inner) => inner,
            Err(arc) => RouterInner {
                path_router: arc.path_router.clone(),
                default_fallback: arc.default_fallback,
            },
        }
    }

    pub fn layer<L>(self, layer: L) -> Self
    where
        L: Layer<Route> + Clone + 'static,
        L::Service: TowerService<Request> + Clone + 'static,
        <L::Service as TowerService<Request>>::Response: IntoResponse + 'static,
        <L::Service as TowerService<Request>>::Error: Into<Infallible> + 'static,
        <L::Service as TowerService<Request>>::Future: 'static,
    {
        let this = self.into_inner();
        Router {
            inner: Rc::new(
                (RouterInner {
                    path_router: this.path_router.layer(layer.clone()),
                    default_fallback: this.default_fallback,
                }),
            ),
        }
    }

    pub fn route_layer<L>(self, layer: L) -> Self
    where
        L: Layer<Route> + Clone + 'static,
        L::Service: TowerService<Request> + Clone + 'static,
        <L::Service as TowerService<Request>>::Response: IntoResponse + 'static,
        <L::Service as TowerService<Request>>::Error: Into<Infallible> + 'static,
        <L::Service as TowerService<Request>>::Future: 'static,
    {
        let this = self.into_inner();
        Router {
            inner: Rc::new(
                (RouterInner {
                    path_router: this.path_router.layer(layer),
                    default_fallback: this.default_fallback,
                }),
            ),
        }
    }
}

struct RouterInner<S> {
    path_router: PathRouter<S>,
    default_fallback: bool,
    // catch_all_fallback: Fallback<S>,
}

enum Fallback<S, E = Infallible> {
    Default(Route<E>),
    Service(Route<E>),
    BoxedHandler(BoxedIntoRoute<S, E>),
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
