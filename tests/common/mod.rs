//! Common test utilities and configuration
//!
//! This module provides shared fixtures and utilities for all integration tests.
//!
//! # Clippy Configuration for Tests
//!
//! Each test file must include these allows at the top (before any other items):
//! ```rust,ignore
//! #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! #![allow(clippy::indexing_slicing, clippy::cast_possible_truncation)]
//! #![allow(clippy::cast_possible_wrap, clippy::needless_update)]
//! #![allow(clippy::items_after_statements, clippy::uninlined_format_args)]
//! #![allow(clippy::assertions_on_constants)]
//! ```

pub mod fixtures;
