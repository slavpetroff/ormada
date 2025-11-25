//! Database connection and transaction types
//!
//! This module provides all database-related types, mirroring Django's `django.db` module.
//! Users should import from here instead of using `sea_orm` directly.
//!
//! # Usage
//!
//! ```rust,ignore
//! use seaorm_django::db::{Database, DatabaseConnection};
//!
//! let db = Database::connect("sqlite::memory:").await?;
//! ```

// Re-export all SeaORM database types
// This allows internal code to use `crate::db::*` instead of `sea_orm::*`

// Connection types
pub use sea_orm::Database;
pub use sea_orm::DatabaseBackend;
pub use sea_orm::DatabaseConnection;
pub use sea_orm::DatabaseTransaction;

// Connection traits
pub use sea_orm::ConnectionTrait;
pub use sea_orm::TransactionTrait;

// Transaction configuration
pub use sea_orm::AccessMode;
pub use sea_orm::IsolationLevel;

// Error type
pub use sea_orm::DbErr;

// Statement for raw SQL
pub use sea_orm::Statement;

// Schema for table creation
pub use sea_orm::Schema;

// Query result types
pub use sea_orm::QueryResult;
