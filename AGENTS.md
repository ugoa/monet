# Agent Guidelines for Monet

This document contains guidelines and commands for agents working in the Monet repository.
Monet is an io-uring based, share-nothing (thread-per-core) web framework with structured concurrency.
It is a Rust workspace with the following crates:
- `monet`: the main web framework
- `monoio`: the async runtime
- `monoio-compat`: compatibility layer for monoio
- Examples in `examples/`

## Build Commands

### Standard Build
```bash
cargo build          # Build all crates in debug mode
cargo build --all    # Explicitly build all workspace members
cargo build --release # Build in release mode
```

### Build Specific Crate
```bash
cargo build -p monet
cargo build -p mondayio    # monoio's package name
cargo build -p mondayio-compat
```

### Check Compilation (No Output)
```bash
cargo check          # Check all crates
cargo check --all
```

## Running Examples
```bash
cargo run --example hello-world
cargo run --example non-static
```

## Linting and Formatting

### Clippy (Linting)
```bash
cargo clippy --all   # Run clippy on all crates
cargo clippy --all -- -D warnings  # Treat warnings as errors
```

Workspace clippy configuration:
- `macro_use_imports = "warn"` (enforced at workspace level)
- `unexpected_cfgs = "warn"` (for rust lints)

Note: The `monet` crate root (`lib.rs`) contains `#![allow(clippy::all)]` and `#![allow(warnings)]` to suppress warnings during development.

### Rustfmt (Formatting)
```bash
cargo fmt --check    # Check formatting without changes
cargo fmt            # Format all code
```

No custom `rustfmt.toml` found; uses default Rust formatting.

## Testing

### Run All Tests
```bash
cargo test --all     # Run all tests in workspace
cargo test --all --release # Run in release mode
```

### Run Tests for Specific Crate
```bash
cargo test -p monet
cargo test -p mondayio
cargo test -p mondayio-compat
```

### Run a Single Test File
```bash
cargo test --test <test_file_name>
```
Example: `cargo test --test tcp_echo` runs `crates/monoio/tests/tcp_echo.rs`.

### Run a Single Test Function
```bash
cargo test <test_function_name>
```
Example: `cargo test echo_server`.

### Test with Specific Features
```bash
cargo test --all --features async-cancel,iouring
```

### Integration Tests
Integration tests are located in `crates/monoio/tests/`. They use the `#[monoio::test_all]` attribute which runs tests on both iouring and legacy drivers (if supported). Variants like `#[monoio::test_all(timer_enabled = true)]` enable timer support.

## Code Style Guidelines

### Edition
- `monet` uses Rust 2024 edition.
- `monoio` and `monoio-compat` use Rust 2021 edition.

### Imports Order
1. Standard library imports (`std::`, `core::`)
2. External crate imports (`http::`, `serde::`, `tower::`)
3. Internal crate imports (`crate::`, `super::`)
4. Re-exports (`pub use`)

Example from `lib.rs`:
```rust
use std::borrow::Cow;
use std::pin::Pin;
use std::task::{Context, Poll};
use http_body_util::BodyExt;
```

### Module Structure
- Each module should be defined in its own file (e.g., `extract/query.rs`).
- Use `pub mod` to expose modules.
- Use `pub(crate)` for internal visibility within the crate.
- Use `#[macro_use]` for macro modules.

### Naming Conventions
- Structs, Enums, Traits: `PascalCase`
- Variables, functions, methods: `snake_case`
- Constants: `SCREAMING_SNAKE_CASE`
- Type parameters: short uppercase (`T`, `S`, `E`, `B`)
- Lifetime parameters: `'a`, `'b`, `'req`

### Error Handling
- Use `Result<T, E>` for fallible operations.
- Use `BoxError` alias (`type BoxError = Box<dyn std::error::Error + Send + Sync>`) for generic errors.
- For infallible cases, use `Infallible` or `std::convert::Infallible`.
- In tests, `unwrap()` and `expect()` are acceptable.
- In production code, prefer proper error propagation and avoid `unwrap()` unless the value is guaranteed (e.g., after a check).

### Derives
- Derive common traits where appropriate: `#[derive(Debug, Clone, Copy, Default)]`.
- Use `#[must_use]` for types where ignoring the result is likely a bug.
- Use `#[allow(clippy::...)]` sparingly; prefer fixing the lint. Module‑level allowances are used in `lib.rs` (`#![allow(clippy::all)]`).

### Async/Await
- Use `async fn` and `.await` for asynchronous operations.
- The runtime is `monoio`; do not use `tokio` runtime directly (except in compatibility layer).
- Use `#[monoio::test_all]` for async tests.

### Macros
- Custom macros are defined in `macros.rs`.
- Use `#[macro_export]` for public macros.
- Use `macro_rules!` for declarative macros.

### Documentation
- Document public APIs with `///` comments.
- Use `// TODO` or `todo!()` for unfinished code.
- Use `//` for inline comments where necessary.

### Formatting Details
- Indentation: 4 spaces (standard Rust).
- Line length: aim for 100 characters, but not strictly enforced.
- Braces: same line for functions, structs, enums.
- Trailing commas: in multi‑line structs, enums, and match arms.

### Type Aliases
- Use type aliases for frequently used complex types (e.g., `HttpRequest<T = Body> = http::Request<T>`).
- Place aliases near the top of the file or in `lib.rs`.

### Generic Constraints
- Write `where` clauses on separate lines if they are long.
- Order constraints: lifetime bounds first, then trait bounds.

Example:
```rust
impl<T, S> FromRequestParts<S> for Query<T>
where
    T: DeserializeOwned,
{
    // ...
}
```

## Commit & Pull Request Guidelines

- Commit messages should be concise and describe the change.
- Use present tense ("Add feature" not "Added feature").
- Prefix with crate name if change is limited (e.g., `monet: fix router panic`).
- Pull requests should include a summary of changes and any breaking changes.

## Additional Notes

- The workspace uses `resolver = "3"` (the latest feature resolver).
- The `monoio` crate has many feature flags; the default set includes `async-cancel`, `bytes`, `iouring`, `legacy`, `macros`, `utils`.
- The `monoio-compat` crate provides a `hyper` feature for compatibility with hyper.
- Examples are located in `examples/` and can be run with `cargo run --example hello-world`.
- No Cursor rules (`.cursor/rules/` or `.cursorrules`) or Copilot rules (`.github/copilot-instructions.md`) are present.

## Troubleshooting

- If you encounter `io-uring` errors, ensure your kernel supports io_uring (Linux 5.6+).
- For legacy driver (mio) support, enable the `legacy` feature.
- Tests may fail on Windows; some are gated with `#[cfg(not(windows))]`.

---
*This file is intended for use by AI agents working in the Monet repository. Keep it up‑to‑date as the project evolves.*