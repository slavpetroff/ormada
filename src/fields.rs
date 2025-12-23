//! Field and column types
//!
//! This module provides field-related types, mirroring Ormada's `ormada.db.models.fields`.
//! Users should import from here instead of using `sea_orm` directly.
//!
//! # Usage
//!
//! ```rust,ignore
//! use ormada::fields::{ColumnTrait, Value};
//!
//! // Column operations
//! let condition = Book::Title.eq("Rust");
//! ```

// Column trait for field operations
pub use sea_orm::ColumnTrait;

// Value types for database values
pub use sea_orm::Value;

// Active value types for model mutations
pub use sea_orm::ActiveValue;
pub use sea_orm::IntoActiveValue;
pub use sea_orm::NotSet;
pub use sea_orm::Set;
pub use sea_orm::Unchanged;

// Expression types for complex queries
pub use sea_orm::sea_query::Expr;
pub use sea_orm::ExprTrait;

// Condition building
pub use sea_orm::Condition;

// Query ordering
pub use sea_orm::Order;

// Join types
pub use sea_orm::JoinType;

// Primary key trait
pub use sea_orm::PrimaryKeyTrait;

// Datetime types (re-exported from chrono)
pub use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, Utc};

/// Type alias for datetime with timezone (`DateTime<FixedOffset>`)
/// This matches `SeaORM's` `DateTimeWithTimeZone` type and Ormada's `DateTimeField`
#[allow(clippy::doc_markdown)]
pub type DateTimeWithTimeZone = DateTime<FixedOffset>;
