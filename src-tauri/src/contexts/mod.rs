//! Bounded-context source layout.
//!
//! Large legacy modules keep their public module paths while their implementations
//! live under this directory. Parent modules use `#[path = ...]` so Rust tooling
//! still parses, formats, and tests the moved sources as normal modules.
