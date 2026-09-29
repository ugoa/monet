# monet

`monet` is an [io-uring](https://en.wikipedia.org/wiki/Io_uring) based, share-nothing (aka. thread-per-core) web framework with structured concurrency.

It is built on top of [compio](https://github.com/compio-rs/compio) for the runtime and
[hyper](https://github.com/hyperium/hyper) for HTTP/1, and exposes an ergonomic,
`axum`-flavored routing and extractor API.

```rust
use std::net::SocketAddr;

use monet::{Request, Router, get};

async fn greeting(_req: Request) -> String {
    "Hello, monet!".to_string()
}

fn main() {
    let addr: SocketAddr = ([0, 0, 0, 0], 9527).into();

    monet::run(addr, || Router::new().at("/", get(greeting)));
}
```

## Highlights

- **io-uring first** — I/O is driven by `compio`, so on Linux it uses io-uring instead of epoll.
- **Thread-per-core / share-nothing** — each worker owns its own runtime and per-thread `Router`,
  avoiding cross-core synchronization on the hot path.
- **Structured concurrency** — in-flight requests, accepted connections, and background tasks are
  all tracked and raced inside a single event loop, so nothing is left dangling.
- **Composable middleware** — plain `async fn(Request, Chain) -> Response` functions, wrapped in
  order with `wrap_by`.
- **Type-safe extractors** — deserialize path params, query strings, forms, and JSON into your
  own types via `serde`.

## Main features

### Runtime & concurrency

- `monet::run(addrs, || Router)` — spawns one worker per CPU core, each with its own router built
  from the factory closure (share-nothing).
- `Server::new(addrs, factory).workers(n).run()` — same, but with an explicit worker count.
- `SingleThreadServer::new(addrs, router).run()` — runs everything on the current thread.
  Useful for tests, embedding, or manually managing threads.
- CPU core affinity per worker (best-effort; not supported on macOS).
- `SO_REUSEPORT` so all workers can bind the same address and let the kernel load-balance.
- `monet::spawn(fut)` — fire-and-forget background tasks that run on the calling worker's runtime
  without blocking request handlers.

### Routing

- `Router::new().at(path, route)` with a `matchit` radix-tree matcher.
- Per-method routing helpers: `get`, `post`, `put`, `patch`, `delete`, `head`, `connect`,
  `options`, `trace`, and `any` (method fallback).
- Chain methods on a single route: `get(a).post(b).any(c)`.
- `Router::nest(prefix, router)` — mount a sub-router under a prefix.
- `Router::merge(other)` — merge two routers (panics on conflicting method handlers).
- `Router::catch(endpoint)` — global fallback for unmatched paths.
- `Router::serve_dir(path, dir)` — static file serving (see below).
- Path syntax supports named params, wildcards, and nesting:
  - `/{id}`, `/{name}`
  - `/{*rest}` catch-all wildcards
  - `req.matched_path()` exposes the matched route pattern.

### Middleware

Middlewares are just async functions `async fn(Request, Chain) -> Response`, so they can run code
before and after `chain.next(req).await`. Applied bottom-up via `wrap_by`:

```rust
Router::new()
    .at("/", get(endpoint))
    .wrap_by(inner)   // runs first
    .wrap_by(outer)   // wraps inner
    .wrap_by(outmost) // wraps outer
```

Built-in middlewares:

- `AttachState<T>` / `Router::with_state(value)` — attach cloneable application state so handlers
  can read it via `req.state::<T>()`.
- `CatchPanic` — catch panics inside the handler chain and turn them into `500 Internal Server
  Error` instead of killing the connection/worker.
- `StripPrefix` — rewrite the request URI when serving a sub-path.

### Extractors

Request payloads are deserialized from `serde`-friendly sources:

| Source        | API                                   | Wrapper type |
| ------------- | ------------------------------------- | ------------ |
| Path params   | `req.path::<T>()`                     | `Path<T>`    |
| Query string  | `req.query_params::<T>()`             | `Query<T>`   |
| Form body     | `req.into_form::<T>().await`          | `Form<T>`    |
| JSON body     | `req.into_json::<T>().await`          | `Json<T>`    |
| Raw body      | `req.into_bytes().await`              | `Bytes`      |
| Matched route | `req.matched_path()`                  | `Option<&Rc<str>>` |
| Raw query     | `req.raw_query()`                     | `Option<String>` |

Answers are returned with `IntoResponse`. Many common types are supported out of the box:
`&'static str`, `String`, `StatusCode`, `(StatusCode, T)`, `Json<T>`, `Form<T>`, `Html<T>`,
`Bytes`/`Vec<u8>`, `HeaderMap`, `()`, and `Result<T, E>`.

### Per-request state

`Request` carries a cloneable, `Send`-free extension map:

```rust
req.add_state(value);          // insert
req.state::<T>();              // read
req.state_mut::<T>();          // mutate
req.remove_state::<T>();       // take
```

This is also how path params and application state flow downstream to handlers.

### Error handling

- `monet::Error` (built with `thiserror`) covers JSON/form/query/path deserialization failures and
  implements `IntoResponse`, mapping each variant to the right status code (400, 422, 415, 500).
- `BodyError` / `BoxError` for body-stream failures.

### Static files

`Router::serve_dir("/assets", "./public")` serves files from a directory with:

- MIME type detection (`mime_guess`),
- directory `index.html` appending and trailing-slash redirects,
- `Last-Modified` response headers,
- `If-Modified-Since` / `If-Unmodified-Since` conditional requests (304/412),
- path traversal protection.

### Observability

Tracing is initialized automatically with `tracing-subscriber`, defaulting to the `monet=trace`
filter and overridable through the `RUST_LOG` environment variable.

## Examples

The [`crates/monet/examples`](crates/monet/examples) directory contains runnable examples:

| Example | What it shows |
| --- | --- |
| `hello-world` | Minimal multi-core server |
| `single-thread` | `SingleThreadServer` and concurrent I/O |
| `simple-middleware` | Before/after middleware and response mutation |
| `with-state` | Shared application state |
| `nested-route`, `nested-route-2` | Nesting routers and nested path params |
| `merge-route` | Merging independent routers |
| `wildcard-route` | Catch-all and matched-path reporting |
| `fallback` | Per-method and global fallbacks |
| `parse-path`, `parse-query`, `parse-form`, `parse-json` | Extractors |
| `serve-html` | Returning HTML |
| `serve-static` | Serving a directory |
| `catch-panic` | Recovering from handler panics |
| `background-task` | Offloading work with `monet::spawn` |

Run one with:

```sh
cargo run --example hello-world
cargo run --example serve-static   # run from the project root
```

## Project layout

```
crates/
  monet/          # the framework
    src/
      server.rs   # workers, core affinity, event loop, background tasks
      router.rs   # routing, nesting, merging, middleware chains
      request.rs  # request parts, extractors, extension state
      response.rs # IntoResponse implementations
      handler/    # Endpoint/Middleware traits + built-in impls
      listener.rs # Listener trait and hyper stream adapter
      error.rs    # error types and status-code mapping
  monet-macros/   # experimental proc-macros (currently disabled)
```

## Status

`monet` is an early-stage project and the API is still evolving. See the
[git history](https://github.com/) for the change log. Known gaps include HTTP/2/3, TLS
termination at the server level, range requests, and a stable middleware extractor story.

## License

MIT
