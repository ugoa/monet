use std::{
    cell::RefCell, convert::Infallible, future::Future, io, net::SocketAddr,
    panic::AssertUnwindSafe, pin::Pin, sync::Arc, thread,
};

use compio::net::{TcpListener, TcpSocket, TcpStream, ToSocketAddrsAsync};
use futures::{
    future::{pending, poll_fn},
    stream::StreamExt,
};
use futures_concurrency::future::{FutureGroup, Race};
use futures_util::{FutureExt, Stream};
use hyper::{server::conn::http1, service::service_fn};

use crate::{
    Router,
    listener::{HyperStream, Listener, any_addrs},
};

thread_local! {
    static BACKGROUND_TASKSET: RefCell<FutureGroup<Pin<Box<dyn Future<Output = ()>>>>> =
        RefCell::new(FutureGroup::new());
}

pub fn spawn<F>(future: F)
where
    F: Future<Output = ()> + 'static, // 'static is required because it's stored in thread_local
{
    BACKGROUND_TASKSET.with(|group| {
        group.borrow_mut().insert(Box::pin(future));
    });
}

enum Event {
    NewConnection { io: TcpStream },
    RequestProcessed,
    BackgroundTaskCompleted,
}

pub fn run<A, F>(addrs: A, router_threadlocal_factory: F)
where
    A: Send + Clone + 'static + ToSocketAddrsAsync,
    F: Send + Sync + 'static + Fn() -> Router,
{
    let core_ids = core_affinity::get_core_ids().expect("to succeed on *nix/win/macos");
    let factory = Arc::new(router_threadlocal_factory);

    let handles = core_ids
        .into_iter()
        .map(|id| {
            let addrs = addrs.clone();
            let factory = Arc::clone(&factory);

            thread::spawn(move || {
                core_affinity::set_for_current(id);
                let router: Router = factory();

                build_service(addrs, router, true);
            })
        })
        .collect::<Vec<_>>();

    for handle in handles.into_iter() {
        handle.join().unwrap();
    }
}

pub fn run_with_single_thread<A>(addrs: A, router: Router)
where
    A: Send + Clone + 'static + ToSocketAddrsAsync,
{
    build_service(addrs, router, false);
}

fn build_service<A>(addrs: A, router: Router, reuse_port: bool)
where
    A: Send + Clone + 'static + ToSocketAddrsAsync,
{
    let app = async {
        let socket: TcpSocket = any_addrs(addrs, |addr| async move {
            let socket = TcpSocket::new_v4().await.expect("succeed");
            socket.set_reuseport(reuse_port).unwrap();
            socket.bind(addr).await.unwrap();
            Ok(socket)
        })
        .await
        .unwrap();

        let mut listener: TcpListener = socket.listen(1024).await.unwrap();

        let mut inflight_requests = FutureGroup::new();

        loop {
            let accept_fut = <TcpListener as Listener>::accept(&mut listener)
                .map(|(io, _)| Event::NewConnection { io });

            let requests_fut = async {
                if !inflight_requests.is_empty() {
                    inflight_requests.next().await;
                    Event::RequestProcessed
                } else {
                    pending().await
                }
            };

            let bg_taskset_fut = async {
                if BACKGROUND_TASKSET.with(|g| !g.borrow().is_empty()) {
                    poll_fn(|cx| {
                        BACKGROUND_TASKSET.with(|g| Pin::new(&mut *g.borrow_mut()).poll_next(cx))
                    })
                    .await;
                    Event::BackgroundTaskCompleted
                } else {
                    pending().await
                }
            };

            match (accept_fut, requests_fut, bg_taskset_fut).race().await {
                Event::NewConnection { io } => {
                    let service = async {
                        http1::Builder::new()
                            .serve_connection(
                                HyperStream::new_plain(io),
                                service_fn(async |req| {
                                    router.dispatch(req.into()).map(Ok::<_, Infallible>).await
                                }),
                            )
                            .await
                    };
                    inflight_requests.insert(AssertUnwindSafe(service).catch_unwind());
                }
                Event::RequestProcessed => (),
                Event::BackgroundTaskCompleted => (),
            }
        }
    };

    let rt = compio::runtime::Runtime::new().expect("shall not fail to create runtime");
    rt.block_on(app);
}
