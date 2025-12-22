//! Schema types and parsing for Ormada ORM migrations
//!
//! This crate provides:
//! - Schema type definitions (`TableSchema`, `ColumnSchema`, etc.)
//! - Source file parsing to extract schema from `#[ormada_model]` and `#[ormada_schema]`
//! - Diff algorithm for generating migrations
//!
//! Used by both `ormada-derive` (proc macros) and `ormada-cli` (migration commands).

pub mod diff;
pub mod parser;
pub mod types;

pub use diff::*;
pub use parser::*;
pub use types::*;
