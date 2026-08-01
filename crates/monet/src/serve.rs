use std::{
    cell::RefCell, convert::Infallible, future::Future, panic::AssertUnwindSafe, pin::Pin,
    sync::Arc, thread,
};

use compio::net::{SocketOpts, TcpListener, TcpStream, ToSocketAddrsAsync};
use futures::{
    future::poll_fn,
    stream::{self, StreamExt},
};
use futures_concurrency::{future::FutureGroup, stream::Merge};
use futures_util::{FutureExt, Stream};
use hyper::{server::conn::http1, service::service_fn};

use crate::{
    NEVEL_FAIL, Router,
    listener::{HyperStream, Listener},
};

thread_local! {
    static BACKGROUND_JOB_GROUP: RefCell<FutureGroup<Pin<Box<dyn Future<Output = ()>>>>> =
        RefCell::new(FutureGroup::new());
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

enum Event {
    NewConnection { io: TcpStream },
    ConnectionDone,
    BackgroundJobDone,
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

                    let group = RefCell::new(FutureGroup::new());

                    let connection_stream = stream::unfold(&mut listener, |listener| async move {
                        let conn = listener.accept().await;
                        Some((Event::NewConnection { io: conn.0 }, listener))
                    });

                    let handler_stream = stream::unfold(&group, |_| async {
                        println!("waiting for new connection...");
                        let _res =
                            poll_fn(|cx| Pin::new(&mut *group.borrow_mut()).poll_next(cx)).await;
                        Some((Event::ConnectionDone, &group))
                    });

                    let bg_stream = stream::unfold((), |()| async {
                        let _res = poll_fn(|cx| {
                            BACKGROUND_JOB_GROUP
                                .with(|g| Pin::new(&mut *g.borrow_mut()).poll_next(cx))
                        })
                        .await;
                        Some((Event::BackgroundJobDone, ()))
                    });

                    let events = (connection_stream, handler_stream, bg_stream).merge();
                    futures_lite::pin!(events);

                    while let Some(event) = events.next().await {
                        match event {
                            Event::NewConnection { io: stream } => {
                                group.borrow_mut().insert(
                                    AssertUnwindSafe(async {
                                        http1::Builder::new()
                                            .serve_connection(
                                                HyperStream::new(stream),
                                                service_fn(async |req| {
                                                    router
                                                        .dispatch(req.into())
                                                        .map(Ok::<_, Infallible>)
                                                        .await
                                                }),
                                            )
                                            .await
                                            .expect(NEVEL_FAIL)
                                    })
                                    .catch_unwind(),
                                );
                            }
                            Event::ConnectionDone => {
                                // A connection future finished. The result was consumed by poll_next.
                                // Handle logging/metrics here if needed.
                            }
                            Event::BackgroundJobDone => {
                                // A background job finished. Handle result if needed.
                            }
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
