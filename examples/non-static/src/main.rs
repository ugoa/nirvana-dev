#![allow(clippy::all)]
#![allow(warnings)]

pub use bytes::Bytes;
use futures::future::Map;
use http::StatusCode;
pub use http_body::Body as HttpBody;
use http_body::Frame;
use http_body_util::BodyExt;
use hyper::server::conn::http1;
use hyper_util::service::TowerToHyperService;
use monet::opaque_future;
use monoio::net::TcpListener;
use monoio_compat::hyper::MonoioIo;
use monoio_compat::{AsyncRead, AsyncWrite, TcpStreamCompat, UnixStreamCompat};
use pin_project_lite::pin_project;
use std::future::Future;
use std::{
    convert::Infallible,
    future::Ready,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll, ready},
};
use tower::ServiceExt;
use tower::service_fn;

pub struct Body(Pin<Box<dyn HttpBody<Data = Bytes, Error = BoxError>>>);
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;
pub type HttpRequest<T = Body> = http::Request<T>;
pub type HttpResponse<T = Body> = http::Response<T>;
pub use tower::Service as TowerService;

#[monoio::main(threads = 1)]
async fn main() {
    let mut tower_service = MapIntoResponse::new(HandlerService::new(hello));

    let thread_id = std::thread::current().id();
    println!("Starting Monoio application on thread: {thread_id:?}",);

    use std::net::SocketAddr;

    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    let mut listener = TcpListener::bind(addr).unwrap();

    loop {
        let (io, remote_addr) = Listener::accept(&mut listener).await;

        let io = monoio_compat::hyper::MonoioIo::new(io);

        let mut hyper_service = TowerToHyperService::new(tower_service.clone());

        monoio::spawn_without_static(async {
            println!("Task started on thread {:?}", std::thread::current().id());
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, hyper_service)
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}

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

impl From<&'static str> for Body {
    fn from(buf: &'static str) -> Self {
        Self::new(http_body_util::Full::from(buf))
    }
}

async fn hello() -> &'static str {
    "No static haha ha"
}

#[derive(Clone)]
pub(crate) struct MapIntoResponse<S> {
    pub inner: S,
}

impl<S> MapIntoResponse<S> {
    pub(crate) fn new(inner: S) -> Self
    where
        S: TowerService<HttpRequest> + Clone,
        S::Response: IntoResponse,
    {
        Self { inner }
    }
}

impl<B, S> TowerService<http::Request<B>> for MapIntoResponse<S>
where
    S: TowerService<http::Request<B>>,
    S::Response: IntoResponse,
{
    type Response = HttpResponse;
    type Error = S::Error;
    type Future = MapIntoResponseFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<B>) -> Self::Future {
        MapIntoResponseFuture {
            inner: self.inner.call(req),
        }
    }
}

pin_project! {
    pub(crate) struct MapIntoResponseFuture<F> {
        #[pin]
        pub inner: F,
    }
}

impl<F, T, E> Future for MapIntoResponseFuture<F>
where
    F: Future<Output = Result<T, E>>,
    T: IntoResponse,
{
    type Output = Result<HttpResponse, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let res = ready!(self.project().inner.poll(cx)?);

        Poll::Ready(Ok(res.into_response()))
    }
}

pub trait Listener: 'static {
    type Io: AsyncRead + AsyncWrite + Unpin;

    type Addr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr);

    fn local_addr(&self) -> std::io::Result<Self::Addr>;
}

impl Listener for monoio::net::TcpListener {
    type Io = monoio_compat::TcpStreamCompat;

    type Addr = std::net::SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            match Self::accept(self).await {
                Ok((stream, addr)) => return (TcpStreamCompat::new(stream), addr),
                Err(e) => todo!(), // handle error
            }
        }
    }

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        Self::local_addr(self)
    }
}

pub trait Handler<X>: Clone + Sized {
    type Future: Future<Output = HttpResponse>;

    fn call(self, req: HttpRequest) -> Self::Future;

    fn with_state(self) -> HandlerService<Self, X> {
        HandlerService::new(self)
    }
}

impl<F, Fut, Res> Handler<((),)> for F
where
    F: FnOnce() -> Fut + Clone,
    Fut: Future<Output = Res>,
    Res: IntoResponse,
{
    type Future = Pin<Box<dyn Future<Output = HttpResponse>>>;

    fn call(self, _req: HttpRequest) -> Self::Future {
        Box::pin(monoio::spawn_without_static(async {
            self().await.into_response()
        }))
    }
}

pub trait IntoResponse {
    /// Create a response.
    fn into_response(self) -> HttpResponse;
}

impl<B> IntoResponse for HttpResponse<B>
where
    B: http_body::Body<Data = bytes::Bytes> + 'static,
    B::Error: Into<BoxError>,
{
    fn into_response(self) -> HttpResponse {
        self.map(Body::new)
    }
}

impl IntoResponse for &'static str {
    fn into_response(self) -> HttpResponse {
        Body::from(self).into_response()
    }
}

impl IntoResponse for Body {
    fn into_response(self) -> HttpResponse {
        HttpResponse::new(self)
    }
}

impl<H, X> HandlerService<H, X> {
    pub fn new(handler: H) -> Self {
        Self {
            handler,
            _marker: PhantomData,
        }
    }
}

impl<H, X> Clone for HandlerService<H, X>
where
    H: Clone,
{
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            _marker: PhantomData,
        }
    }
}

pub struct HandlerService<H, X> {
    pub handler: H,
    pub(crate) _marker: PhantomData<fn() -> X>,
}

impl<H, X, B> TowerService<HttpRequest<B>> for HandlerService<H, X>
where
    H: Handler<X> + Clone + 'static,
    B: HttpBody<Data = bytes::Bytes> + 'static,
    B::Error: Into<BoxError>,
{
    type Response = HttpResponse;

    type Error = Infallible;

    type Future = IntoServiceFuture<H::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: HttpRequest<B>) -> Self::Future {
        use futures_util::future::FutureExt;
        let req = req.map(Body::new);
        let handler = self.handler.clone();

        let future = Handler::call(handler, req);

        let future = future.map(Ok as _);

        IntoServiceFuture::new(future)
    }
}

opaque_future! {
    /// The response future for [`IntoService`](super::IntoService).
    pub type IntoServiceFuture<F> =
        Map<
            F,
            fn(HttpResponse) -> Result<HttpResponse, Infallible>,
        >;
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
