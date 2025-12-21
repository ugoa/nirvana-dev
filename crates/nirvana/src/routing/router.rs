use crate::prelude::*;
use crate::routing::method_router::MethodRouter;
use crate::routing::path_router::{Endpoint, Node, RouteId};
use crate::routing::route_tower::RouteFuture;
use crate::{handler::Handler, routing::route::BoxedIntoRoute};
use matchit::MatchError;
use std::rc::Rc;
use std::{collections::HashMap, convert::Infallible};
use tower::Layer;

// #[derive(Clone)]
// pub struct SimpleRouter<S = ()> {
//     routes: Vec<MethodRouter<S>>,
//     node: Node,
// }

#[must_use]
#[derive(Clone)]
pub struct Router<S = ()> {
    pub routes: Vec<Endpoint<S>>,
    pub node: Node,
    pub default_fallback: bool,
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
            routes: Default::default(),
            node: Default::default(),
            default_fallback: true,
        }
    }

    pub fn route(mut self, path: &str, method_router: MethodRouter<S>) -> Self {
        if let Some(route_id) = self.node.path_to_route_id.get(path) {
            if let Some(Endpoint::MethodRouter(prev_method_router)) = self.routes.get(route_id.0) {
                let service = Endpoint::MethodRouter(
                    prev_method_router
                        .clone()
                        .merge_for_path(Some(path), method_router)
                        .unwrap(),
                );
                self.routes[route_id.0] = service;
            }
        } else {
            let endpoint = Endpoint::MethodRouter(method_router);
            self.new_route(path, endpoint).unwrap();
        }

        self
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

    pub fn layer<L>(self, layer: L) -> Self
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
            default_fallback: self.default_fallback,
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
        let routes = self
            .routes
            .into_iter()
            .map(|endpoint| endpoint.layer(layer.clone()))
            .collect();

        Self {
            routes,
            node: self.node,
            default_fallback: self.default_fallback,
        }
    }

    pub fn with_state<S2>(self, state: S) -> Router<S2> {
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
        }
    }

    pub(crate) fn call_with_state(
        &self,
        req: Request,
        state: S,
    ) -> Result<RouteFuture<Infallible>, (Request, S)> {
        let (mut parts, body) = req.into_parts();

        match self.node.at(parts.uri.path()) {
            Ok(matched) => {
                let route_id = matched.value;

                let endpoint = self.routes.get(route_id.0).expect(
                    "It is granted a valid route for id. Please file an issue if it is not",
                );

                let req = Request::from_parts(parts, body);

                match endpoint {
                    Endpoint::MethodRouter(method_router) => {
                        Ok(method_router.call_with_state(req, state))
                    }
                    Endpoint::Route(route) => Ok(route.clone().call_owned(req)),
                }
            }
            Err(MatchError::NotFound) => Err((Request::from_parts(parts, body), state)),
        }
    }
}

enum Fallback<S, E = Infallible> {
    Default(Route<E>),
    Service(Route<E>),
    BoxedHandler(BoxedIntoRoute<S, E>),
}

// impl<S> SimpleRouter<S>
// where
//     S: Clone + 'static,
// {
//     pub fn new() -> Self {
//         Self {
//             routes: Default::default(),
//             node: Default::default(),
//         }
//     }
//
//     pub fn route(mut self, path: &str, method_router: MethodRouter<S>) -> Self {
//         if let Some(route_id) = self.node.path_to_route_id.get(path) {
//             if let Some(prev_method_router) = self.routes.get(route_id.0) {
//                 // merge to existing router
//                 todo!()
//             }
//         } else {
//             let new_route_id = RouteId(self.routes.len());
//             self.node.insert(path, new_route_id);
//             self.routes.push(method_router);
//         }
//
//         self
//     }
//
//     pub fn with_state<S2>(&self, state: S) -> SimpleRouter<S2> {
//         let method_routers = (0..self.routes.len())
//             .map(|i| self.routes[i].clone().with_state(state.clone()))
//             .collect();
//
//         let node = self.node.clone();
//         SimpleRouter {
//             routes: method_routers,
//             node: node,
//         }
//     }
//
//     pub(crate) fn call_with_state(&self, req: Request, state: S) -> RouteFuture<Infallible> {
//         let (parts, body) = req.into_parts();
//
//         let matched = self.node.at(parts.uri.path()).unwrap();
//
//         let id = *matched.value;
//
//         let endpoint = self.routes.get(id.0).unwrap();
//
//         let req = Request::from_parts(parts, body);
//         endpoint.call_with_state(req, state)
//     }
// }
