use std::{
    borrow::Cow,
    future::Future,
    io,
    mem::MaybeUninit,
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll, ready},
};

use compio::{
    io::{AsyncRead, AsyncWrite, util::Splittable},
    net::{TcpListener, TcpStream, UnixListener, UnixStream},
    tls::{MaybeTlsStream, TlsStream},
};
use compio_net::ToSocketAddrsAsync;
use send_wrapper::SendWrapper;
use tracing::error;

/// Types that can listen for connections.
pub trait Listener: 'static {
    /// The listener's IO type.
    // type Io: AsyncRead + AsyncWrite + Unpin + 'static;
    type Io: Splittable + 'static;

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

pub async fn any_addrs<T, F: Future<Output = io::Result<T>>>(
    addrs: impl ToSocketAddrsAsync,
    f: impl Fn(SocketAddr) -> F,
) -> io::Result<T> {
    let addrs = addrs.to_socket_addrs_async().await?;
    let mut last_err = None;
    for addr in addrs {
        match f(addr).await {
            Ok(l) => return Ok(l),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "could not resolve to any addresses",
        )
    }))
}

/// A stream wrapper for hyper.
#[derive(Debug)]
pub struct HyperStream<S: Splittable>(SendWrapper<MaybeTlsStream<S>>);

impl<S: Splittable> HyperStream<S> {
    /// Create a new [`HyperStream`] from a plain stream.
    pub fn new_plain(s: S) -> Self {
        Self(SendWrapper::new(MaybeTlsStream::new_plain(s)))
    }

    /// Create a new [`HyperStream`] from a TLS stream.
    pub fn new_tls(s: TlsStream<S>) -> Self {
        Self(SendWrapper::new(MaybeTlsStream::new_tls(s)))
    }

    /// Whether the stream is TLS-encrypted.
    pub fn is_tls(&self) -> bool {
        self.0.is_tls()
    }
}

impl<S: Splittable + 'static> HyperStream<S>
where
    S::ReadHalf: AsyncRead + Unpin,
    S::WriteHalf: AsyncWrite + Unpin,
{
    /// Returns the negotiated ALPN protocol.
    pub fn negotiated_alpn(&self) -> Option<Cow<'_, [u8]>> {
        self.0.negotiated_alpn()
    }
}

impl<S: Splittable + 'static> hyper::rt::Read for HyperStream<S>
where
    S::ReadHalf: AsyncRead + Unpin,
    S::WriteHalf: AsyncWrite + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        mut buf: hyper::rt::ReadBufCursor<'_>,
    ) -> Poll<io::Result<()>> {
        let uninit = unsafe { buf.as_mut() };
        uninit.fill(MaybeUninit::new(0));
        let res = ready!(futures_util::AsyncRead::poll_read(
            Pin::new(&mut *self.0),
            cx,
            unsafe { uninit.assume_init_mut() }
        ))?;
        unsafe { buf.advance(res) };
        Poll::Ready(Ok(()))
    }
}

impl<S: Splittable + 'static> futures_util::AsyncRead for HyperStream<S>
where
    S::ReadHalf: AsyncRead + Unpin,
    S::WriteHalf: AsyncWrite + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        futures_util::AsyncRead::poll_read(Pin::new(&mut *self.0), cx, buf)
    }

    fn poll_read_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [io::IoSliceMut<'_>],
    ) -> Poll<io::Result<usize>> {
        futures_util::AsyncRead::poll_read_vectored(Pin::new(&mut *self.0), cx, bufs)
    }
}

impl<S: Splittable + 'static> hyper::rt::Write for HyperStream<S>
where
    S::ReadHalf: AsyncRead + Unpin,
    S::WriteHalf: AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        futures_util::AsyncWrite::poll_write(Pin::new(&mut *self.0), cx, buf)
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        futures_util::AsyncWrite::poll_write_vectored(Pin::new(&mut *self.0), cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        true
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        futures_util::AsyncWrite::poll_flush(Pin::new(&mut *self.0), cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        futures_util::AsyncWrite::poll_close(Pin::new(&mut *self.0), cx)
    }
}

impl<S: Splittable + 'static> futures_util::AsyncWrite for HyperStream<S>
where
    S::ReadHalf: AsyncRead + Unpin,
    S::WriteHalf: AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        futures_util::AsyncWrite::poll_write(Pin::new(&mut *self.0), cx, buf)
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        futures_util::AsyncWrite::poll_write_vectored(Pin::new(&mut *self.0), cx, bufs)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        futures_util::AsyncWrite::poll_flush(Pin::new(&mut *self.0), cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        futures_util::AsyncWrite::poll_close(Pin::new(&mut *self.0), cx)
    }
}
