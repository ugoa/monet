use std::{
    cell::RefCell, convert::Infallible, future::Future, panic::AssertUnwindSafe, pin::Pin,
    sync::Arc, thread,
};

use compio::net::{TcpListener, TcpSocket, TcpStream, ToSocketAddrsAsync};
use futures::{
    future::{pending, poll_fn},
    stream::StreamExt,
};
use futures_concurrency::future::{FutureGroup, Race};
use futures_util::{FutureExt, Stream};
use hyper::{server::conn::http1, service::service_fn};
use socket2::{Domain, SockAddr};

use crate::{
    Router,
    listener::{HyperStream, Listener, any_addrs},
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

pub fn run<A, F>(addrs: A, router_threadlocal_factory: F)
where
    A: Send + Clone + 'static + ToSocketAddrsAsync,
    F: Send + Sync + 'static + Fn() -> Router,
{
    let core_ids = core_affinity::get_core_ids().expect("shall succeed on supported platforms");
    let factory = Arc::new(router_threadlocal_factory);

    let handles = core_ids
        .into_iter()
        .map(|core_id| {
            let addrs = addrs.clone();
            let factory = Arc::clone(&factory);

            thread::spawn(move || {
                //  Won't work on macos. https://developer.apple.com/forums/thread/44002
                core_affinity::set_for_current(core_id);
                let router: Router = factory();

                build_service(addrs, router, true);
            })
        })
        .collect::<Vec<_>>();

    for handle in handles.into_iter() {
        handle.join().expect("threads shall join just fine");
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
        let mut inflight_requests = FutureGroup::new();

        let socket: TcpSocket = any_addrs(addrs, |addr| async move {
            let sa = SockAddr::from(addr);
            let socket: TcpSocket = match sa.domain() {
                Domain::IPV4 => TcpSocket::new_v4().await,
                Domain::IPV6 => TcpSocket::new_v6().await,
                _ => panic!("Unsupported Domain"),
            }
            .expect("shall create TcpSocket successfully");
            socket.set_reuseport(reuse_port).unwrap();
            socket.bind(addr).await.unwrap();
            Ok(socket)
        })
        .await
        .unwrap();
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
