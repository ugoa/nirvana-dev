#![allow(clippy::all)]
#![allow(warnings)]

pub mod extract;
pub mod handler;
pub mod routing;

mod prelude {
    pub use crate::{
        Body, BoxError, HttpBody, HttpRequest, IntoResponse, Request, Response, Route, TowerService,
    };
}

pub use self::{
    extract::state::State, response::IntoResponse, routing::method_routing::get,
    routing::route::Route, routing::router::SimpleRouter, serve::serve,
};

#[macro_use]
pub(crate) mod macros;

pub mod response;

pub mod serve;

use std::borrow::Cow;
use std::pin::Pin;
use std::task::{Context, Poll};

pub use bytes::Bytes;
pub use http_body::{Body as HttpBody, Frame};
pub use tower::Service as TowerService;

use http_body_util::BodyExt;

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
}

impl From<Cow<'static, str>> for Body {
    fn from(buf: Cow<'static, str>) -> Self {
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

pub use http::Request as HttpRequest;

pub type Request<T = Body> = HttpRequest<T>;

pub use http::Response as HttpResponse;

pub type Response<T = Body> = HttpResponse<T>;
