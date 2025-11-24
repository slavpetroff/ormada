// Integration tests are allowed to use unwrap/expect for clarity
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(unused_must_use)]

// Common test utilities
#[path = "common/mod.rs"]
mod common;

// Ergonomic hooks test - demonstrates the user-facing API
#[path = "lifecycle_hooks/test_ergonomic_hooks.rs"]
mod test_ergonomic_hooks;
