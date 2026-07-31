use std::{
    cell::RefCell,
    convert::Infallible,
    future::Future,
    net::SocketAddr,
    ops::DerefMut,
    panic::AssertUnwindSafe,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll, ready},
    thread,
};

use compio::{
    io::{AsyncRead, AsyncWrite, compat::AsyncStream},
    net::{SocketOpts, TcpListener, TcpStream, ToSocketAddrsAsync, UnixListener, UnixStream},
};
use futures::{future::poll_fn, stream::StreamExt};
use futures_concurrency::future::FutureGroup;
use futures_util::{FutureExt, Stream};
use hyper::{server::conn::http1, service::service_fn};
use send_wrapper::SendWrapper;

use crate::{NEVEL_FAIL, Router};

type BgFut = Pin<Box<dyn Future<Output = ()>>>;

thread_local! {
    static BACKGROUND_JOB_GROUP: RefCell<FutureGroup<BgFut>> = RefCell::new(FutureGroup::new());
}

pub fn spawn_task<F>(future: F)
where
    F: Future<Output = ()> + 'static, // 'static is required because it's stored in thread_local
{
    BACKGROUND_JOB_GROUP.with(|group| {
        group.borrow_mut().insert(Box::pin(future));
    });
}

pub fn run<A, F>(addrs: A, threadlocal_router_factory: F)
where
    A: ToSocketAddrsAsync + Send + 'static + Clone,
    F: Send + Sync + 'static + Fn() -> Router,
{
    let core_ids = core_affinity::get_core_ids().expect("To succeed on *nix/win/macos platform");
    let factory = Arc::new(threadlocal_router_factory);

    let handles = core_ids
        .into_iter()
        .map(|id| {
            let addr = addrs.clone();
            let factory = Arc::clone(&factory);

            thread::spawn(move || {
                core_affinity::set_for_current(id);
                let router = factory();
                let app = async {
                        let soc_opts = SocketOpts::default().reuse_port(true);
                        let mut listener = TcpListener::bind_with_options(addr, &soc_opts)
                            .await
                            .unwrap();

                        let mut group = FutureGroup::new();
                        loop {
                            tokio::select! {

                                biased;

                                stream = listener.accepts() => {
                                    group.insert(AssertUnwindSafe(async {
                                        http1::Builder::new()
                                            .serve_connection(
                                                HyperStream::new(stream.0),
                                                service_fn(async |req| {
                                                    router.dispatch(req.into()).map(Ok::<_, Infallible>).await
                                                }),
                                            )
                                            .await
                                            .expect(NEVEL_FAIL)
                                    }).catch_unwind());
                                },

                                _ =  group.next(), if !group.is_empty()  => (),

                                _ = poll_fn(|cx| {
                                    BACKGROUND_JOB_GROUP.with(|g| {
                                        let mut group_ref = g.borrow_mut();
                                        Pin::new(&mut *group_ref).poll_next(cx)
                                    })
                                }), if !BACKGROUND_JOB_GROUP.with(|g| g.borrow().is_empty()) => (),
                            }
                        }
                    };

                    let rt = compio::runtime::Runtime::new().expect("shall not fail to create runtime");
                    rt.block_on(app);
            })
        })
        .collect::<Vec<_>>();

    for handle in handles.into_iter() {
        handle.join().unwrap();
    }
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
