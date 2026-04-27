use std::{
    cell::RefCell,
    convert::Infallible,
    future::Future,
    io::{self},
    net::SocketAddr,
    ops::DerefMut,
    pin::Pin,
    task::{Context, Poll, ready},
};

use bytes::Bytes;
use compio::{
    io::{AsyncRead, AsyncWrite, compat::AsyncStream},
    net::{TcpListener, TcpStream, UnixListener, UnixStream},
};
use futures::stream::StreamExt;
use futures_concurrency::future::FutureGroup;
use http_body_util::Full;
use hyper::{
    Method, Request, Response, StatusCode, body::Incoming, server::conn::http1, service::service_fn,
};
use send_wrapper::SendWrapper;

use crate::Router;

/// Types that can listen for connections.
pub trait Listener: 'static {
    /// The listener's IO type.
    type Io: AsyncRead + AsyncWrite + Unpin + 'static;

    /// The listener's address type.
    type Addr;

    /// Accept a new incoming connection to this listener.
    ///
    /// If the underlying accept call can return an error, this function must
    /// take care of logging and retrying.
    fn accepts(&mut self) -> impl Future<Output = (Self::Io, Self::Addr)>;

    /// Returns the local address that this listener is bound to.
    fn local_addr(&self) -> io::Result<Self::Addr>;
}

impl Listener for TcpListener {
    type Addr = SocketAddr;
    type Io = TcpStream;

    async fn accepts(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            match Self::accept(self).await {
                Ok(tup) => return tup,
                Err(_e) => todo!(), // handle error
            }
        }
    }

    fn local_addr(&self) -> io::Result<Self::Addr> {
        Self::local_addr(self)
    }
}

impl Listener for UnixListener {
    type Addr = socket2::SockAddr;
    type Io = UnixStream;

    async fn accepts(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            match Self::accept(self).await {
                Ok(tup) => return tup,
                Err(_e) => todo!(), // handle error
            }
        }
    }

    fn local_addr(&self) -> io::Result<Self::Addr> {
        Self::local_addr(self)
    }
}

/// A stream wrapper for hyper.
pub struct HyperStream<S>(SendWrapper<AsyncStream<S>>);

impl<S> HyperStream<S> {
    /// Create a hyper stream wrapper.
    pub fn new(s: S) -> Self {
        Self(SendWrapper::new(AsyncStream::new(s)))
    }

    /// Get the reference of the inner stream.
    pub fn get_ref(&self) -> &S {
        self.0.get_ref()
    }
}

impl<S> std::fmt::Debug for HyperStream<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HyperStream").finish_non_exhaustive()
    }
}

impl<S: AsyncRead + Unpin + 'static> hyper::rt::Read for HyperStream<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        mut buf: hyper::rt::ReadBufCursor<'_>,
    ) -> Poll<io::Result<()>> {
        let stream = unsafe { self.map_unchecked_mut(|this| this.0.deref_mut()) };
        let slice = unsafe { buf.as_mut() };
        let len = ready!(stream.poll_read_uninit(cx, slice))?;
        unsafe { buf.advance(len) };
        Poll::Ready(Ok(()))
    }
}

impl<S: AsyncWrite + Unpin + 'static> hyper::rt::Write for HyperStream<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let stream = unsafe { self.map_unchecked_mut(|this| this.0.deref_mut()) };
        futures_util::AsyncWrite::poll_write(stream, cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let stream = unsafe { self.map_unchecked_mut(|this| this.0.deref_mut()) };
        futures_util::AsyncWrite::poll_flush(stream, cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let stream = unsafe { self.map_unchecked_mut(|this| this.0.deref_mut()) };
        futures_util::AsyncWrite::poll_close(stream, cx)
    }
}

pub fn serve(addr: SocketAddr, router: Router) {
    let cache = RefCell::new(0);
    let app = async {
        let mut listener = compio::net::TcpListener::bind(addr).await.unwrap();
        let mut group = FutureGroup::new();
        loop {
            tokio::select! {
                biased;
                stream = listener.accepts() => {
                    println!("Received at {}", jiff::Timestamp::now());
                    group.insert(async {
                        http1::Builder::new()
                            .serve_connection(
                                HyperStream::new(stream.0),
                                service_fn(async |req| router.run(req).await),
                            )
                            .await
                            .expect("Should handle request successfully")
                    });
                },
                _ =  group.next(), if !group.is_empty()  => (),
            }
        }
    };
    let rt = compio::runtime::Runtime::new().expect("cannot create runtime");
    rt.block_on(app);
}

async fn handle_request(stream: compio::net::TcpStream, cache: &RefCell<i32>) {
    http1::Builder::new()
        .serve_connection(
            HyperStream::new(stream),
            service_fn(async |req| action(req, &cache).await),
        )
        .await
        .expect("Should handle request successfully");
}

async fn action(
    req: Request<Incoming>,
    cache: &RefCell<i32>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") => {
            compio::runtime::time::sleep(std::time::Duration::from_millis(2000)).await;
            *cache.borrow_mut() += 1;

            use jiff::Zoned;

            Ok(Response::new(Full::new(Bytes::from(format!(
                "Visit Count: {} at {} \n",
                *cache.borrow(),
                Zoned::now()
            )))))
        }
        (&Method::GET, "/compio") => Ok(Response::new(Full::new(Bytes::from("Hello Compio!")))),
        _ => Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Full::new(Bytes::from("404 not found")))
            .unwrap()),
    }
}
