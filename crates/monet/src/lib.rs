#![allow(clippy::all)]
#![allow(warnings)]

pub use self::{
    extract::state::State, response::IntoResponse, routing::method_router::get,
    routing::route::Route, routing::router::Router, serve::serve,
};
pub use bytes::Bytes;
pub use http_body::{Body as HttpBody, Frame};
use http_body_util::BodyExt;
use std::borrow::Cow;
use std::pin::Pin;
use std::task::{Context, Poll};
pub use tower::Service as TowerService;

pub mod extract;
pub mod handler;
pub mod handler_tower_impl;
pub mod response;
pub mod routing;
pub mod serve;
#[macro_use]
pub(crate) mod macros;
pub(crate) mod prelude {
    pub use crate::{
        Body, BoxError, HttpBody, HttpRequest, HttpResponse, IntoResponse, Route, TowerService,
    };
    pub use std::fmt;
}

pub type HttpRequest<T = Body> = http::Request<T>;
pub type HttpResponse<T = Body> = http::Response<T>;
pub use tower::util::MapResponseLayer;

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

pub struct Body(Pin<Box<dyn HttpBody<Data = Bytes, Error = BoxError>>>);

impl Body {
    pub fn new<B>(http_body: B) -> Self
    where
        B: HttpBody<Data = Bytes> + 'static,
        B::Error: Into<BoxError>,
    {
        let body = http_body.map_err(Into::into);
        Body(Box::pin(body))
    }

    pub fn empty() -> Self {
        Self::new(http_body_util::Empty::new())
    }
}

impl From<Cow<'static, str>> for Body {
    fn from(buf: Cow<'static, str>) -> Self {
        Self::new(http_body_util::Full::from(buf))
    }
}

impl From<String> for Body {
    fn from(buf: String) -> Self {
        Self::new(http_body_util::Full::from(buf))
    }
}

impl From<Vec<u8>> for Body {
    fn from(buf: Vec<u8>) -> Self {
        Self::new(http_body_util::Full::from(buf))
    }
}

impl From<&'static [u8]> for Body {
    fn from(buf: &'static [u8]) -> Self {
        Self::new(http_body_util::Full::from(buf))
    }
}

impl From<&'static str> for Body {
    fn from(buf: &'static str) -> Self {
        Self::new(http_body_util::Full::from(buf))
    }
}

impl From<Cow<'static, [u8]>> for Body {
    fn from(buf: Cow<'static, [u8]>) -> Self {
        Self::new(http_body_util::Full::from(buf))
    }
}

impl From<Bytes> for Body {
    fn from(buf: Bytes) -> Self {
        Self::new(http_body_util::Full::from(buf))
    }
}

impl http_body::Body for Body {
    type Data = Bytes;
    type Error = BoxError;

    #[inline]
    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        // self.inner.poll_frame(cx)
        Pin::new(&mut self.0).poll_frame(cx)
    }

    #[inline]
    fn size_hint(&self) -> http_body::SizeHint {
        self.0.size_hint()
    }

    #[inline]
    fn is_end_stream(&self) -> bool {
        self.0.is_end_stream()
    }
}
