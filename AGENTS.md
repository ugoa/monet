# Agent Guidelines for Monet

This document contains guidelines and commands for agents working in the Monet repository.
Monet is an io-uring based, share-nothing (thread-per-core) web framework with structured concurrency.
It is a Rust workspace with the following crates:
- `monet`: the main web framework
- `monoio`: the async runtime
- `monoio-compat`: compatibility layer for monoio
- Examples in `examples/`

## Build/Lint/Test Commands

### Building
- `cargo build` - Build all workspace crates (monet, monoio, monoio-compat)
- `cargo build --release` - Build optimized release version
- `cargo check` - Fast compilation check without building
- `cargo build -p monet` - Build specific crate (monet, monoio, or monoio-compat)

### Testing
- `cargo test` - Run all tests across all workspace crates
- `cargo test --workspace` - Explicitly run tests in all workspace members
- `cargo test -p crate_name` - Run tests for a specific crate
- `cargo test test_function_name` - Run specific test by name
- `cargo test --lib` - Run only library tests (not examples/integration)
- `cargo test --tests` - Run only integration tests

### Linting
- `cargo clippy` - Run clippy linter for all workspace crates
- `cargo clippy --fix` - Automatically apply clippy suggestions where possible
- `cargo fmt` - Format all Rust code according to standard style
- `cargo fmt --check` - Check if code is properly formatted

### Running Examples
- `cargo run -p hello-world` - Run the hello-world example
- `cargo run -p hello-world --release` - Run example in release mode

## Code Style Guidelines

### Imports
- Use explicit imports rather than glob imports where possible
- Group imports: standard library, external crates, then local crate modules
- Use `use crate::module::submodule` for internal crate imports
- Use `use crate::prelude::*` where appropriate for common imports
- Follow alphabetical ordering within import groups

### Formatting
- Use standard Rust formatting (via `cargo fmt`)
- 4 space indentation
- Maximum line length of 100 characters (soft limit)
- Always use braces for blocks, even single-line control flow
- Place opening braces on the same line as the control statement
- Use consistent spacing around operators

### Naming Conventions
- Structs, Enums, Traits: `PascalCase`
- Functions, Methods, Variables: `snake_case`
- Constants: `UPPER_SNAKE_CASE`
- Type Parameters: `PascalCase` (preferably single uppercase letter like `T`, `S`)
- Modules: `snake_case`

### Types
- Use explicit type annotations for public API functions
- Prefer concrete types over generic types in private code when clear
- Use `BoxError` (Box<dyn std::error::Error + Send + Sync>) for error types
- Use `Result<T, BoxError>` for fallible operations that cross API boundaries
- Leverage Rust's type system for compile-time safety (e.g., newtypes)

### Error Handling
- Use `Result<T, E>` for recoverable errors
- Use `panic!` for programming errors/bugs (not user input errors)
- Use `BoxError` as the standard error type for generic error handling
- Handle errors explicitly rather than using `unwrap()` in production code
- For TODO implementations, use `todo!()` or `unimplemented!()`

### Documentation
- Document all public items with `///` doc comments
- Include example code in documentation where beneficial
- Use `//!` for module-level documentation
- Follow Rust documentation conventions (summary line, detailed description)

### Testing
- Write unit tests for all public functions
- Use `#[cfg(test)]` for test modules
- Test both success and failure cases
- Use descriptive test function names that indicate what is being tested
- Use property-based testing for complex functions when appropriate
- Run `cargo build` to make sure it compiles successfully

### Performance
- Leverage async/await for I/O-bound operations
- Use I/O-uring based operations for high-performance I/O
- Consider memory allocation patterns and prefer reusable buffers
- Profile performance-critical paths using appropriate tools
- Be mindful of zero-copy operations where possible

### Async Patterns
- The runtime is `monoio`; do not use `tokio` runtime directly (except in compatibility layer).
- Follow structured concurrency patterns
- Use appropriate async primitives from the monoio ecosystem
- Leverage thread-per-core architecture for scalability
- the 'static bound should be avoided as much as possible
- Use `async fn` and `.await` for asynchronous operations.

### Code Organization
- Group related functionality in modules
- Maintain clear separation between extractors, handlers, routing, and response logic
- Follow the existing architecture patterns in the codebase
- Use `pub(crate)` for internal API visibility when appropriate
- Use feature flags for optional functionality

### Specific to monet Framework
- Follow Tower service and layer patterns
- Use the provided `Router`, `Route`, and `MethodRouter` types
- Implement handlers using the `Handler` trait
- Use extractors from the `extract` module for request data
- Leverage the state management system for shared application state
