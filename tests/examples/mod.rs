//! Ormada Examples as Tests
//!
//! This module contains runnable examples demonstrating all Ormada features.
//! Each example includes assertions to verify correctness.
//!
//! Run all example tests with:
//! ```sh
//! cargo test examples::
//! ```

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(clippy::indexing_slicing)]

pub mod aggregations;
pub mod basic_crud;
pub mod bulk_operations;
pub mod filtering;
pub mod fk_validation;
pub mod group_by_aggregations;
pub mod many_to_many;
pub mod one_to_one;
pub mod projections;
pub mod query_debugging;
pub mod relation_loading;
pub mod relations;
pub mod soft_delete;
pub mod streaming;
pub mod transactions;
pub mod upsert_operations;
