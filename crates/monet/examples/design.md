
In the Axum framework, it uses Tokio as a runtime. Because Tokio adopts work-stealing approach, It forces the future to be Send & Sync,
which forces the user-defined state also has to be Send and Sync because it is being used by the future,which creates a complexity where users have to wrap their states in Arc and Mutex.

I'm trying to build a thread per core Rust Async web framework, with structured concurrency, The plan is to have each thread manage its own runtime so that the future does not needs to be synced across multi-threads, which eliminates the need of sync and send. I also want to use the FutureGroup from future-concurrency crate as the root of all the incoming requests to achieve a tree-structured concurrency, I also plan to use scoped threads so that I can share reference directly in multiple threads, so that the user-defined state does not need to be wrapped in Arc and Mutex.I also want to provide an elegant way for users to access the state they defined in the main body from the handler function.It could be something similar like Axum's extractor pattern.


As the user of the monet framework, they shall be able to write code like this:
```rust
use std::{collections::HashMap, net::SocketAddr, sync::Mutex, thread};

use monet::{Request, Response, Router, State, get, response::*};
use tracing::info;

async fn greeting(_req: Request, State(ctx): State<&MyContext>) -> Response {
    format!("Current thread ID: {:?}", thread::current().id()).into_response()
}

#[derive(Default)]
struct MyContext {
    s: String,
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();
    info!("Server running at: {}", addr);

    let ctx = MyContext {
        s: "Global".to_string(),
    };

    monet::Server::new(addr, || {
        Router::new().at("/", get(greeting)).at(
            "/hi",
            get(async |_req: Request| format!("closure works too")),
        )
    })
    .run(&ctx);
}

Notice that the ctx is passed in as a reference.

```


To support the above client-defined code, I have created few components as below.


```rust
type RouteId = usize;

#[derive(Default, Debug)]
pub struct Router {
    matcher: matchit::Router<RouteId>,
    id_to_path: HashMap<RouteId, Rc<str>>,
    routes: Vec<Route>,
    fallback: Option<Rc<dyn Endpoint>>,
}

#[derive(Debug)]
pub enum Route {
    MethodRoute(MethodRoute),
    Service(Layers),
}

#[derive(Default, Debug)]
pub struct MethodRoute {
    pub map: HashMap<Method, Layers>,
    pub fallback: Option<Rc<dyn Endpoint>>,
}

#[derive(Debug, Clone)]
pub struct Layers(Rc<RefCell<SharedLayers>>);

#[derive(Debug, Clone)]
pub struct SharedLayers {
    pub endpoint: Rc<dyn Endpoint>,
    pub middlewares: Vec<Rc<dyn Middleware>>,
}

#[derive(Default)]
pub struct Server<F, A> {
    router_factory: Arc<F>,
    socket_addrs: A,
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
        }
    }

    pub fn run<A, F, S>(self, state: &S)
    where
        A: Send + Clone + 'static + ToSocketAddrsAsync,
        F: Send + Sync + 'static + Fn() -> NewRouter<S>,
        S: Sync,
    {
        let core_ids = core_affinity::get_core_ids().expect("shall succeed on supported platforms");
        let router_factory = Arc::new(self.router_threadlocal_factory);

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
                                                    router.dispatch(req.into(), state)
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

}
```

Please advise how should I continue from here and finish the design for the Handler trait that can accept state as input parameters. It can be a function that accepts two parameters. One is a request, another one is a state.

