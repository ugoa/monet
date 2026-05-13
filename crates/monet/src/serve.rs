use std::{
    convert::Infallible,
    future::Future,
    net::SocketAddr,
    ops::DerefMut,
    panic::AssertUnwindSafe,
    pin::Pin,
    task::{Context, Poll, ready},
};

use compio::{
    io::{AsyncRead, AsyncWrite, compat::AsyncStream},
    net::{TcpListener, TcpStream, UnixListener, UnixStream},
};
use futures::{FutureExt, stream::StreamExt};
use futures_concurrency::future::FutureGroup;
use futures_util::FutureExt;
use hyper::{server::conn::http1, service::service_fn};
use send_wrapper::SendWrapper;

use crate::Router;

pub fn run(addr: SocketAddr, router: Router) {
    // dbg!(&router);
    let app = async {
        let mut listener = compio::net::TcpListener::bind(addr).await.unwrap();
        let mut group = FutureGroup::new();
        loop {
            tokio::select! {
                biased;
                stream = listener.accepts() => {
                    println!("Received at {}", jiff::Timestamp::now());
                    group.insert(AssertUnwindSafe(async {
                        http1::Builder::new()
                            .serve_connection(
                                HyperStream::new(stream.0),
                                service_fn(async |req| router.handle(req.into()).await ),
                            )
                            .await
                            .expect("Should handle request successfully")
                    }).catch_unwind());
                },
                _ =  group.next(), if !group.is_empty()  => (),
            }
        }
    };
    let rt = compio::runtime::Runtime::new().expect("cannot create runtime");
    rt.block_on(app);
}

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
    fn local_addr(&self) -> std::io::Result<Self::Addr>;
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

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
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

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
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
    ) -> Poll<std::io::Result<()>> {
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
    ) -> Poll<std::io::Result<usize>> {
        let stream = unsafe { self.map_unchecked_mut(|this| this.0.deref_mut()) };
        futures_util::AsyncWrite::poll_write(stream, cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let stream = unsafe { self.map_unchecked_mut(|this| this.0.deref_mut()) };
        futures_util::AsyncWrite::poll_flush(stream, cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let stream = unsafe { self.map_unchecked_mut(|this| this.0.deref_mut()) };
        futures_util::AsyncWrite::poll_close(stream, cx)
    }
}
