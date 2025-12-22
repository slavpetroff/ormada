//! Migration system for Ormada ORM
//!
//! This module provides the infrastructure for database migrations using
//! the same declarative syntax as `#[ormada_model]`.
//!
//! # Overview
//!
//! Ormada migrations use `#[ormada_schema]` to define schema snapshots that
//! the CLI parses to generate SQL migrations. This approach:
//!
//! - Uses the **same syntax** as `#[ormada_model]` - no new DSL to learn
//! - Supports **delta migrations** via `extends` for concise change definitions
//! - Allows **data migrations** using the standard Ormada ORM API
//!
//! # Migration File Structure
//!
//! Each migration file contains schema definitions wrapped in a module named
//! after the migration ID:
//!
//! ```rust,ignore
//! // migrations/m001_initial.rs
//! use ormada::migration::prelude::*;
//!
//! pub mod m001_initial {
//!     use super::*;
//!
//!     #[ormada_schema(table = "authors", migration = "m001_initial")]
//!     pub struct Author {
//!         #[primary_key]
//!         pub id: i32,
//!         pub name: String,
//!     }
//!
//!     #[ormada_schema(table = "books", migration = "m001_initial")]
//!     pub struct Book {
//!         #[primary_key]
//!         pub id: i32,
//!         #[foreign_key(Author)]
//!         pub author_id: i32,
//!         pub title: String,
//!     }
//! }
//! ```
//!
//! # Delta Migrations
//!
//! For subsequent migrations, use `extends` to only specify changes:
//!
//! ```rust,ignore
//! // migrations/m002_add_isbn.rs
//! use ormada::migration::prelude::*;
//!
//! pub mod m002_add_isbn {
//!     use super::*;
//!
//!     #[ormada_schema(table = "books", migration = "m002_add_isbn", after = "m001_initial", extends = Book)]
//!     pub struct Book {
//!         // Only new fields - inherited fields are implicit
//!         #[index]
//!         pub isbn: String,
//!     }
//! }
//! ```
//!
//! # Rename and Drop Operations
//!
//! ```rust,ignore
//! pub mod m003_refactor {
//!     use super::*;
//!
//!     #[ormada_schema(table = "authors", migration = "m003_refactor", after = "m002_add_isbn", extends = Author)]
//!     pub struct Author {
//!         // Rename a column
//!         #[rename(from = "name", to = "full_name")]
//!         pub full_name: String,
//!
//!         // Add new column
//!         #[unique]
//!         pub email: String,
//!
//!         // Drop a column
//!         #[drop]
//!         pub legacy_field: (),
//!     }
//! }
//! ```
//!
//! # Data Migrations
//!
//! Use `#[ormada_data_migration]` for data transformations:
//!
//! ```rust,ignore
//! use ormada::migration::prelude::*;
//! use crate::models::Author;
//!
//! #[ormada_data_migration(migration = "m004_populate_emails", after = "m003_refactor")]
//! async fn populate_emails(db: &DatabaseConnection) -> Result<(), OrmadaError> {
//!     Author::objects(db)
//!         .filter(Author::Email.is_null())
//!         .update_all(|author| {
//!             author.email = format!("{}@example.com", author.full_name.to_lowercase());
//!         })
//!         .await?;
//!     Ok(())
//! }
//! ```
//!
//! # CLI Commands
//!
//! ```bash
//! # Generate migration from model changes
//! ormada migrate make "description"
//!
//! # Show pending migrations
//! ormada migrate status
//!
//! # Apply pending migrations
//! ormada migrate run
//!
//! # Rollback last migration
//! ormada migrate rollback
//!
//! # Generate SQL without applying (for review)
//! ormada migrate sql
//! ```

/// Prelude for migration files
///
/// Import this in your migration files:
/// ```rust,ignore
/// use ormada::migration::prelude::*;
/// ```
pub mod prelude {
    // Re-export database types
    pub use crate::db::{DatabaseConnection, DbErr};
    pub use crate::error::OrmadaError;

    // Re-export field types commonly used in schemas
    pub use crate::fields::DateTimeWithTimeZone;

    // Re-export the migration macros
    #[cfg(feature = "derive")]
    pub use ormada_derive::{ormada_data_migration, ormada_schema};
}

/// Marker attribute for columns that should be dropped
///
/// Used in delta migrations to indicate a column should be removed.
///
/// # Example
///
/// ```rust,ignore
/// #[ormada_schema(table = "books", migration = "002", extends = Book)]
/// pub struct Book {
///     #[drop]
///     pub legacy_field: (),
/// }
/// ```
#[allow(dead_code)]
pub struct Drop;

/// Marker attribute for column renames
///
/// Used in delta migrations to rename a column.
///
/// # Example
///
/// ```rust,ignore
/// #[ormada_schema(table = "authors", migration = "002", extends = Author)]
/// pub struct Author {
///     #[rename(from = "name", to = "full_name")]
///     pub full_name: String,
/// }
/// ```
#[allow(dead_code)]
pub struct Rename {
    /// Original column name
    pub from: &'static str,
    /// New column name
    pub to: &'static str,
}
