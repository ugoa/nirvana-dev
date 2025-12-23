use hyper::body::Incoming;
use hyper_util::service::TowerToHyperService;
use std::convert::Infallible;
use std::fmt::Debug;
use std::marker::PhantomData;
use tower::ServiceExt;

use hyper::server::conn::http1;
use monoio_compat::hyper::MonoioIo;
use monoio_compat::{AsyncRead, AsyncWrite, TcpStreamCompat, UnixStreamCompat};

use crate::Body;
use crate::HttpBody;
use crate::{BoxError, HttpRequest, HttpResponse, TowerService};

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

impl Listener for monoio::net::UnixListener {
    type Io = monoio_compat::UnixStreamCompat;

    type Addr = monoio::net::unix::SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            match Self::accept(self).await {
                Ok((stream, addr)) => return (UnixStreamCompat::new(stream), addr),
                Err(e) => todo!(), // handle error
            }
        }
    }

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        Self::local_addr(self)
    }
}

#[derive(Debug)]
pub struct IncomingStream<'a, L>
where
    L: Listener,
{
    io: &'a MonoioIo<L::Io>,
    remote_addr: L::Addr,
}

pub fn serve<L, M, S, B>(listener: L, make_service: M) -> Serve<L, M, S, B>
where
    L: Listener,
    M: for<'a> TowerService<IncomingStream<'a, L>, Response = S, Error = Infallible>,
    S: TowerService<HttpRequest, Response = HttpResponse<B>, Error = Infallible> + Clone + 'static,
    B: HttpBody + 'static,
    B::Error: Into<BoxError>,
{
    Serve {
        listener,
        make_service,
        _marker: PhantomData,
    }
}

pub struct Serve<L, M, S, B> {
    listener: L,
    make_service: M,
    _marker: PhantomData<fn(B) -> S>,
}

impl<L, M, S, B> Serve<L, M, S, B>
where
    L: Listener,
    L::Addr: Debug,
    M: for<'a> TowerService<IncomingStream<'a, L>, Response = S, Error = Infallible>,
    S: TowerService<HttpRequest, Response = HttpResponse<B>, Error = Infallible> + Clone + 'static,
    B: HttpBody + 'static,
    B::Error: Into<BoxError>,
{
    async fn run(self) -> ! {
        let Self {
            mut listener,
            mut make_service,
            _marker,
        } = self;

        loop {
            let (io, remote_addr) = listener.accept().await;

            let io = monoio_compat::hyper::MonoioIo::new(io);

            make_service
                .ready()
                .await
                .unwrap_or_else(|err| match err {});

            let tower_service = make_service
                .call(IncomingStream {
                    io: &io,
                    remote_addr,
                })
                .await
                .unwrap_or_else(|err| match err {})
                .map_request(|req: HttpRequest<Incoming>| req.map(Body::new));

            let hyper_service = TowerToHyperService::new(tower_service);

            monoio::spawn_without_static(async move {
                println!("Task started on thread {:?}", std::thread::current().id());
                if let Err(err) = http1::Builder::new()
                    .timer(monoio_compat::hyper::MonoioTimer)
                    .serve_connection(io, hyper_service)
                    .await
                {
                    println!("Error serving connection: {:?}", err);
                }
            });
        }
    }
}

impl<L, M, S, B> IntoFuture for Serve<L, M, S, B>
where
    L: Listener,
    L::Addr: std::fmt::Debug,
    M: for<'a> TowerService<IncomingStream<'a, L>, Response = S, Error = Infallible> + 'static,
    S: TowerService<HttpRequest, Response = HttpResponse<B>, Error = Infallible> + Clone + 'static,
    B: HttpBody + 'static,
    B::Error: Into<BoxError>,
{
    type Output = std::io::Result<()>;

    type IntoFuture = ServeFuture;

    fn into_future(self) -> Self::IntoFuture {
        ServeFuture(Box::pin(async move { self.run().await }))
    }
}

use std::{
    future::Future,
    io,
    pin::Pin,
    task::{Context, Poll},
};

pub struct ServeFuture(futures_core::future::LocalBoxFuture<'static, io::Result<()>>);

impl Future for ServeFuture {
    type Output = io::Result<()>;

    #[inline]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.0.as_mut().poll(cx)
    }
}

impl std::fmt::Debug for ServeFuture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServeFuture").finish_non_exhaustive()
    }
}
