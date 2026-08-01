use std::{
    cell::RefCell, convert::Infallible, future::Future, panic::AssertUnwindSafe, pin::Pin,
    sync::Arc, thread,
};

use compio::net::{SocketOpts, TcpListener, TcpStream, ToSocketAddrsAsync};
use futures::{future::poll_fn, stream::StreamExt};
use futures_concurrency::future::{FutureGroup, Race};
use futures_util::{FutureExt, Stream};
use hyper::{server::conn::http1, service::service_fn};

use crate::{
    NEVEL_FAIL, Router,
    listener::{HyperStream, Listener},
};

thread_local! {
    static BACKGROUND_TASKSET: RefCell<FutureGroup<Pin<Box<dyn Future<Output = ()>>>>> =
        RefCell::new(FutureGroup::new());
}

pub fn spawn_task<F>(future: F)
where
    F: Future<Output = ()> + 'static, // 'static is required because it's stored in thread_local
{
    BACKGROUND_TASKSET.with(|group| {
        group.borrow_mut().insert(Box::pin(future));
    });
}

pub fn run2<A, F>(addrs: A, threadlocal_router_factory: F)
where
    A: Send + Clone + 'static + ToSocketAddrsAsync,
    F: Send + Sync + 'static + Fn() -> Router,
{
    let core_ids = core_affinity::get_core_ids().expect("to succeed on *nix/win/macos");
    let factory = Arc::new(threadlocal_router_factory);

    let handles = core_ids
        .into_iter()
        .map(|id| {
            let addrs = addrs.clone();
            let factory = Arc::clone(&factory);

            thread::spawn(move || {
                core_affinity::set_for_current(id);
                let router: Router = factory();
                let app = async {
                    let mut listener = TcpListener::bind_with_options(
                        addrs,
                        &SocketOpts::default().reuse_port(true),
                    )
                    .await
                    .expect("to bind address successfully");

                    let mut group = FutureGroup::new();

                    loop {
                        tokio::select! {
                            stream = <TcpListener as Listener>::accept(&mut listener) => {
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
                                BACKGROUND_TASKSET.with(|g| {
                                    let mut group_ref = g.borrow_mut();
                                    Pin::new(&mut *group_ref).poll_next(cx)
                                })
                            }), if !BACKGROUND_TASKSET.with(|g| g.borrow().is_empty()) => (),
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
enum Event {
    NewConnection { io: TcpStream },
    RequestProcessed,
    BackgroundTaskCompleted,
}

pub fn run<A, F>(addrs: A, threadlocal_router_factory: F)
where
    A: Send + Clone + 'static + ToSocketAddrsAsync,
    F: Send + Sync + 'static + Fn() -> Router,
{
    let core_ids = core_affinity::get_core_ids().expect("to succeed on *nix/win/macos");
    let factory = Arc::new(threadlocal_router_factory);

    let handles = core_ids
        .into_iter()
        .map(|id| {
            let addrs = addrs.clone();
            let factory = Arc::clone(&factory);

            thread::spawn(move || {
                core_affinity::set_for_current(id);
                let router: Router = factory();
                let app = async {
                    let mut listener = TcpListener::bind_with_options(
                        addrs,
                        &SocketOpts::default().reuse_port(true),
                    )
                    .await
                    .expect("to bind address successfully");

                    let mut group = FutureGroup::new();

                    let accept_fut = <TcpListener as Listener>::accept(&mut listener)
                        .map(|(io, _)| Event::NewConnection { io });

                    let inflight_request_futures = async {
                        if !group.is_empty() {
                            group.next().await;
                            Event::RequestProcessed
                        } else {
                            futures::future::pending().await
                        }
                    };
                    let bg_fut = async {
                        if BACKGROUND_TASKSET.with(|g| !g.borrow().is_empty()) {
                            poll_fn(|cx| {
                                BACKGROUND_TASKSET
                                    .with(|g| Pin::new(&mut *g.borrow_mut()).poll_next(cx))
                            })
                            .await;
                            Event::BackgroundTaskCompleted
                        } else {
                            futures::future::pending().await
                        }
                    };

                    match (accept_fut, inflight_request_futures, bg_fut).race().await {
                        Event::NewConnection { io } => {
                            let service = async {
                                http1::Builder::new()
                                    .serve_connection(
                                        HyperStream::new(io),
                                        service_fn(async |req| {
                                            router
                                                .dispatch(req.into())
                                                .map(Ok::<_, Infallible>)
                                                .await
                                        }),
                                    )
                                    .await
                                    .expect(NEVEL_FAIL)
                            };
                            group.insert(AssertUnwindSafe(service).catch_unwind());
                        }
                        Event::RequestProcessed => (),
                        Event::BackgroundTaskCompleted => (),
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
