use std::{
    cell::RefCell, convert::Infallible, future::Future, marker::PhantomData,
    panic::AssertUnwindSafe, pin::Pin, rc::Rc, sync::Arc, thread,
};

use compio::net::{TcpListener, TcpSocket, TcpStream, ToSocketAddrsAsync};
use core_affinity::CoreId;
use futures::{
    future::{pending, poll_fn},
    stream::StreamExt,
};
use futures_concurrency::future::{FutureGroup, Race};
use futures_util::{FutureExt, Stream};
use hyper::{server::conn::http1, service::service_fn};
use socket2::{Domain, SockAddr};
use tracing::trace;

use crate::{
    IntoResponse, Request, Response, Router,
    handler::NewEndpoint,
    listener::{HyperStream, Listener, any_addrs},
    logging::try_enable_tracing,
};

thread_local! {
    static BACKGROUND_TASKSET: RefCell<FutureGroup<Pin<Box<dyn Future<Output = ()>>>>> =
        RefCell::new(FutureGroup::new());
}

const BACKLOG: i32 = 1024;

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

#[derive(Default)]
pub struct Server<F, A> {
    router_factory: Arc<F>,
    socket_addrs: A,
    workers: usize,
}

impl<F, A, S> Server<F, A, S>
where
    A: Send + Clone + 'static + ToSocketAddrsAsync,
    F: Send + Sync + 'static + Fn() -> Router,
{
    pub fn new(addrs: A, factory: F) -> Self {
        Self {
            router_factory: Arc::new(factory),
            socket_addrs: addrs,
            workers: 0,
        }
    }

    pub fn workers(mut self, num: usize) -> Self {
        self.workers = num;
        self
    }

    pub fn run(&mut self, state: S) {
        try_enable_tracing();

        let mut core_ids = core_affinity::get_core_ids().expect("no reason to fail");
        if self.workers > 0 && self.workers < core_ids.len() {
            core_ids.truncate(self.workers);
        }

        thread::scope(|scope| {
            core_ids.into_iter().for_each(|core_id| {
                let addrs = self.socket_addrs.clone();
                let factory = Arc::clone(&self.router_factory);

                scope.spawn(move || {
                    trace!("Starting Worker thread {:?} ", core_id.id);

                    // Not supported on macOS.
                    core_affinity::set_for_current(core_id);

                    let router: Router = factory();

                    build_service(addrs, router, true);
                });
            });
        });
    }
}

#[derive(Default)]
pub struct NewRouter<S> {
    routes: Vec<Rc<dyn NewEndpoint<S>>>,
    _marker: PhantomData<fn() -> S>,
}

impl<S> NewRouter<S> {
    fn new(h: impl NewEndpoint<S> + 'static) -> Self {
        let n = Rc::new(h);
        Self {
            routes: vec![n],
            _marker: Default::default(),
        }
    }
}

pub fn run2<A, F, S>(addrs: A, router_threadlocal_factory: F, state: &S)
where
    A: Send + Clone + 'static + ToSocketAddrsAsync,
    F: Send + Sync + 'static + Fn() -> NewRouter<S>,
    S: Sync,
{
    let core_ids = core_affinity::get_core_ids().expect("shall succeed on supported platforms");
    let router_factory = Arc::new(router_threadlocal_factory);

    thread::scope(|s| {
        core_ids.into_iter().for_each(|core_id| {
            let rf = Arc::clone(&router_factory);
            let nid = core_id.id;
            trace!("Starting Worker thread {:?} ", nid);
            s.spawn(move || {
                //  Won't work on macos. https://developer.apple.com/forums/thread/44002
                core_affinity::set_for_current(core_id);

                let router = rf();

                let app = async {
                    let mut inflight_requests = FutureGroup::new();

                    let mut listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();

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
                                    BACKGROUND_TASKSET
                                        .with(|g| Pin::new(&mut *g.borrow_mut()).poll_next(cx))
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
                                        .keep_alive(true)
                                        .serve_connection(
                                            HyperStream::new_plain(io),
                                            service_fn(async |req| {
                                                router.routes[0]
                                                    .call(req.into(), state)
                                                    .map(Ok::<_, Infallible>)
                                                    .await
                                            }),
                                        )
                                        .with_upgrades()
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
            });
        });
    });
}

pub fn run<A, RF, SF, S>(addrs: A, router_threadlocal_factory: RF, state_factory: SF)
where
    A: Send + Clone + 'static + ToSocketAddrsAsync,
    RF: Send + Sync + 'static + Fn() -> NewRouter<S>,
    SF: Send + Sync + 'static + Fn() -> S,
{
    try_enable_tracing();

    let core_ids = core_affinity::get_core_ids().expect("shall succeed on supported platforms");
    let router_factory = Arc::new(router_threadlocal_factory);
    let state_factory = Arc::new(state_factory);

    thread::scope(|scope| {
        core_ids.into_iter().for_each(|core_id| {
            let addrs = addrs.clone();
            let rf = Arc::clone(&router_factory);
            let sf = Arc::clone(&state_factory);

            scope.spawn(move || {
                trace!("Starting Worker thread {:?} ", core_id.id);

                //  Won't work on macos. https://developer.apple.com/forums/thread/44002
                core_affinity::set_for_current(core_id);

                let router = rf();
                let state = sf();

                let app = async {
                    let mut inflight_requests = FutureGroup::new();

                    let socket: TcpSocket = any_addrs(addrs, |addr| async move {
                        let sa = SockAddr::from(addr);
                        let socket: TcpSocket = match sa.domain() {
                            Domain::IPV4 => TcpSocket::new_v4().await,
                            Domain::IPV6 => TcpSocket::new_v6().await,
                            _ => panic!("Unsupported Domain"),
                        }
                        .expect("should create TcpSocket successfully");
                        socket.set_reuseport(true).expect("should not fail");
                        socket.bind(addr).await.expect("should not fail");
                        Ok(socket)
                    })
                    .await
                    .unwrap();

                    trace!(
                        "Starting HTTP server at {:?}",
                        &socket
                            .local_addr()
                            .expect("should be resolved as valid address")
                    );

                    let mut listener: TcpListener = socket.listen(BACKLOG).await.unwrap();

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
                                    BACKGROUND_TASKSET
                                        .with(|g| Pin::new(&mut *g.borrow_mut()).poll_next(cx))
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
                                        .keep_alive(true)
                                        .serve_connection(
                                            HyperStream::new_plain(io),
                                            service_fn(async |req| {
                                                router.routes[0]
                                                    .call(req.into(), &state)
                                                    .map(Ok::<_, Infallible>)
                                                    .await
                                            }),
                                        )
                                        .with_upgrades()
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
            });
        });
    });
}

fn build_service<A>(addrs: A, router: Router, reuse_port: bool)
where
    A: Send + Clone + 'static + ToSocketAddrsAsync,
{
    let app = async {
        let mut inflight_requests = FutureGroup::new();

        let socket: TcpSocket = any_addrs(addrs, |addr| async move {
            let sa = SockAddr::from(addr);
            let socket: TcpSocket = match sa.domain() {
                Domain::IPV4 => TcpSocket::new_v4().await,
                Domain::IPV6 => TcpSocket::new_v6().await,
                _ => panic!("Unsupported Domain"),
            }
            .expect("should create TcpSocket successfully");
            socket.set_reuseport(reuse_port).expect("should not fail");
            socket.bind(addr).await.expect("should not fail");
            Ok(socket)
        })
        .await
        .unwrap();

        trace!(
            "Starting HTTP server at {:?}",
            &socket
                .local_addr()
                .expect("should be resolved as valid address")
        );

        let mut listener: TcpListener = socket.listen(BACKLOG).await.unwrap();

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
                            .keep_alive(true)
                            .serve_connection(
                                HyperStream::new_plain(io),
                                service_fn(async |req| {
                                    router.dispatch(req.into()).map(Ok::<_, Infallible>).await
                                }),
                            )
                            .with_upgrades()
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
