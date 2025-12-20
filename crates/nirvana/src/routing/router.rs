use crate::prelude::*;
use crate::routing::method_router::MethodRouter;
use crate::routing::path_router::{Endpoint, Node, PathRouter, RouteId};
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
    pub path_router: PathRouter<S>,
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
            path_router: Default::default(),
            default_fallback: true,
        }
    }

    pub fn route(mut self, path: &str, method_router: MethodRouter<S>) -> Self {
        match (self.path_router.route(path, method_router)) {
            Ok(x) => x,
            Err(err) => panic!("{err}"),
        };
        self
    }

    pub fn layer<L>(self, layer: L) -> Self
    where
        L: Layer<Route> + Clone + 'static,
        L::Service: TowerService<Request> + Clone + 'static,
        <L::Service as TowerService<Request>>::Response: IntoResponse + 'static,
        <L::Service as TowerService<Request>>::Error: Into<Infallible> + 'static,
        <L::Service as TowerService<Request>>::Future: 'static,
    {
        Router {
            path_router: self.path_router.layer(layer.clone()),
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
        Router {
            path_router: self.path_router.layer(layer),
            default_fallback: self.default_fallback,
        }
    }

    pub fn with_state<S2>(self, state: S) -> Router<S2> {
        let path_router = {
            let this = self.path_router;

            let routes = this
                .routes
                .into_iter()
                .map(|endpoint| match endpoint {
                    Endpoint::MethodRouter(method_router) => {
                        Endpoint::MethodRouter(method_router.with_state(state.clone()))
                    }
                    Endpoint::Route(route) => Endpoint::Route(route),
                })
                .collect();
            PathRouter {
                routes,
                node: this.node,
            }
        };
        Router {
            path_router: path_router,
            default_fallback: self.default_fallback,
        }
    }

    pub(crate) fn call_with_state(
        &self,
        req: Request,
        state: S,
    ) -> Result<RouteFuture<Infallible>, (Request, S)> {
        let (mut parts, body) = req.into_parts();

        match self.path_router.node.at(parts.uri.path()) {
            Ok(matched) => {
                let route_id = matched.value;

                let endpoint = self.path_router.routes.get(route_id.0).expect(
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

struct RouterInner<S> {
    pub path_router: PathRouter<S>,
    pub default_fallback: bool,
    // catch_all_fallback: Fallback<S>,
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
