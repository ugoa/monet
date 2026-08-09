use std::{
    future::Future,
    net::SocketAddr,
    ops::DerefMut,
    pin::Pin,
    task::{Context, Poll, ready},
};

use compio::{
    io::{AsyncRead, AsyncWrite, compat::AsyncStream},
    net::{TcpListener, TcpStream, UnixListener, UnixStream},
};
use send_wrapper::SendWrapper;
use tracing::error;

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
    fn accept(&mut self) -> impl Future<Output = (Self::Io, Self::Addr)>;

    /// Returns the local address that this listener is bound to.
    fn local_addr(&self) -> std::io::Result<Self::Addr>;
}

impl Listener for TcpListener {
    type Addr = SocketAddr;
    type Io = TcpStream;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            match Self::accept(self).await {
                Ok(tup) => return tup,
                Err(e) => error!("accept error: {e}"),
            }
        }
    }

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        Self::local_addr(self)
    }
}

#[cfg(unix)]
impl Listener for UnixListener {
    type Addr = socket2::SockAddr;
    type Io = UnixStream;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            match Self::accept(self).await {
                Ok(tup) => return tup,
                Err(e) => error!("accept error: {e}"),
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
