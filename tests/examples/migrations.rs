//! Migration Examples - Schema definitions and data migrations
//!
//! Demonstrates the migration system using `#[ormada_schema]` and `#[ormada_data_migration]`.
//!
//! Note: Migration files are standalone - no `pub mod` wrapping needed since each file
//! is parsed independently by the CLI. The struct names can be the same across migrations
//! because they're in separate files.
//!
//! IMPORTANT: When using `extends`, you must use the SCHEMA model name (e.g., `Author001`)
//! not the ORM model name. In real migration files, each file would have its own `Author`
//! struct, so you'd use `extends = Author` referencing the previous migration's schema.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(unused_must_use)]

use ormada::prelude::*;

// ============================================================================
// Example: How migrations would look in separate files
// ============================================================================

// File: migrations/m001_initial.rs
// ---------------------------------
// Each migration file is standalone - no module wrapping needed.
// In a real project, this would just be `Author` and `Book`.

#[ormada_schema(table = "authors", migration = "m001_initial")]
pub struct Author001 {
    #[primary_key]
    pub id: i32,

    #[max_length(100)]
    pub name: String,

    #[max_length(200)]
    pub email: String,
}

#[ormada_schema(table = "books", migration = "m001_initial")]
pub struct Book001 {
    #[primary_key]
    pub id: i32,

    #[max_length(200)]
    pub title: String,

    // In real migration: #[foreign_key(Author)] - uses schema model from same file
    #[foreign_key(Author001)]
    pub author_id: i32,

    pub price: i32,

    pub published: bool,
}

// File: migrations/m002_add_isbn.rs
// ---------------------------------
// Delta migration - uses `extends` to reference the SCHEMA model from previous migration.
// In a real file: `extends = Book` (the schema model name, not ORM model)
// The CLI resolves this by looking at the `after` migration's schemas.

#[ormada_schema(
    table = "books",
    migration = "m002_add_isbn",
    after = "m001_initial",
    extends = Book001  // In real file: extends = Book (schema model from m001)
)]
pub struct Book002 {
    #[index]
    #[max_length(13)]
    pub isbn: String,

    #[default(0)]
    pub page_count: i32,
}

// File: migrations/m003_refactor_authors.rs
// -----------------------------------------
// Rename and add columns

#[ormada_schema(
    table = "authors",
    migration = "m003_refactor_authors",
    after = "m002_add_isbn",
    extends = Author001
)]
pub struct Author003 {
    #[rename(from = "name")]
    pub full_name: String,

    #[unique]
    pub website: String,
}

// File: migrations/m004_add_categories.rs
// ---------------------------------------
// New table + FK to existing table

#[ormada_schema(
    table = "categories",
    migration = "m004_add_categories",
    after = "m003_refactor_authors"
)]
pub struct Category {
    #[primary_key]
    pub id: i32,

    #[max_length(50)]
    #[unique]
    pub name: String,

    pub description: String,
}

#[ormada_schema(
    table = "books",
    migration = "m004_add_categories",
    after = "m003_refactor_authors",
    extends = Book002
)]
pub struct Book004 {
    #[foreign_key(Category)]
    #[nullable]
    pub category_id: Option<i32>,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_migration_schema_compiles() {
        assert_eq!(Author001::__ORMADA_SCHEMA_TABLE, "authors");
        assert_eq!(Author001::__ORMADA_SCHEMA_MIGRATION, "m001_initial");
        assert_eq!(Book001::__ORMADA_SCHEMA_TABLE, "books");
    }

    #[test]
    fn test_delta_migration_has_extends() {
        assert_eq!(Book002::__ORMADA_SCHEMA_TABLE, "books");
        assert_eq!(Book002::__ORMADA_SCHEMA_MIGRATION, "m002_add_isbn");
        assert_eq!(Book002::__ORMADA_SCHEMA_AFTER, "m001_initial");
        assert_eq!(Book002::__ORMADA_SCHEMA_EXTENDS, "Book001");
    }

    #[test]
    fn test_migration_ordering() {
        assert_eq!(Author001::__ORMADA_SCHEMA_AFTER, "");
        assert_eq!(Book002::__ORMADA_SCHEMA_AFTER, "m001_initial");
        assert_eq!(Author003::__ORMADA_SCHEMA_AFTER, "m002_add_isbn");
        assert_eq!(Category::__ORMADA_SCHEMA_AFTER, "m003_refactor_authors");
    }

    #[test]
    fn test_new_table_in_later_migration() {
        assert_eq!(Category::__ORMADA_SCHEMA_TABLE, "categories");
        assert_eq!(Category::__ORMADA_SCHEMA_EXTENDS, "");
    }

    #[test]
    fn test_multiple_tables_same_migration() {
        // m004 has both Category and Book004
        assert_eq!(Category::__ORMADA_SCHEMA_MIGRATION, "m004_add_categories");
        assert_eq!(Book004::__ORMADA_SCHEMA_MIGRATION, "m004_add_categories");
    }
}
