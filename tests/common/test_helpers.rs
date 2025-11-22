//! Shared test helpers and utilities
//!
//! This module provides common functionality for all tests to avoid duplication.

use chrono::{DateTime, FixedOffset, TimeZone, Utc};
use sea_orm::{Database, DatabaseConnection, DbBackend};
use seaorm_django::prelude::*;

/// Create an in-memory SQLite database for testing
pub async fn setup_test_db() -> DatabaseConnection {
    Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to test database")
}

/// Execute raw SQL on the database (for table creation, etc.)
pub async fn execute_sql(db: &DatabaseConnection, sql: &str) {
    use sea_orm::ConnectionTrait;

    db.execute_unprepared(sql).await.expect("Failed to execute SQL");
}

/// Create a fixed UTC timestamp for testing
pub fn test_timestamp() -> DateTime<FixedOffset> {
    FixedOffset::east_opt(0).unwrap().from_utc_datetime(&Utc::now().naive_utc())
}

/// Create a timestamp N hours ago
pub fn test_timestamp_hours_ago(hours: i64) -> DateTime<FixedOffset> {
    use chrono::Duration;
    let now = Utc::now();
    FixedOffset::east_opt(0)
        .unwrap()
        .from_utc_datetime(&(now - Duration::hours(hours)).naive_utc())
}

/// Macro to create test table SQL
#[macro_export]
macro_rules! create_table_sql {
    ($table_name:expr, $($column:expr),+ $(,)?) => {
        format!(
            "CREATE TABLE IF NOT EXISTS {} ({})",
            $table_name,
            vec![$($column),+].join(", ")
        )
    };
}

/// Common table columns
pub mod columns {
    pub const ID_AUTO: &str = "id INTEGER PRIMARY KEY AUTOINCREMENT";
    pub const VALUE_INT: &str = "value INTEGER NOT NULL";
    pub const NAME_TEXT: &str = "name TEXT NOT NULL";
    pub const CREATED_AT: &str = "created_at TEXT NOT NULL";
    pub const INT_VALUE_NULL: &str = "int_value INTEGER";
    pub const CATEGORY_INT: &str = "category INTEGER NOT NULL";
}

/// Test data factory trait
pub trait TestFactory {
    /// Create a test instance with default values
    fn test_default() -> Self;

    /// Create a test instance with specific ID
    fn test_with_id(id: i32) -> Self;
}

/// Assertion helpers
pub mod assertions {
    /// Assert that two values are approximately equal (for floats)
    pub fn assert_approx_eq(a: f64, b: f64, epsilon: f64) {
        assert!(
            (a - b).abs() < epsilon,
            "Values not approximately equal: {} vs {} (epsilon: {})",
            a,
            b,
            epsilon
        );
    }

    /// Assert that a result is an error
    pub fn assert_is_error<T, E>(result: Result<T, E>) {
        assert!(result.is_err(), "Expected error but got Ok");
    }
}

// Tests are in integration tests, not unit tests for test helpers
