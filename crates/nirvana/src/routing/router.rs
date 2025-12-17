use crate::opaque_future;
use crate::prelude::*;
use crate::routing::method_routing::MethodRouter;
use crate::routing::route_tower_impl::RouteFuture;
use crate::{HttpRequest, extract::FromRequest, handler::Handler, routing::route::Route};
use futures_util::future::Map;
use http::Method;
use matchit::MatchError;
use pin_project_lite::pin_project;
use std::{
    collections::HashMap,
    convert::Infallible,
    fmt,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll, ready},
};
use tower::ServiceExt;
use tower::util::Oneshot;

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
