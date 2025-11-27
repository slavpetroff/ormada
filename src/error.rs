//! Error types for ormada
//!
//! This module provides the error type that all ORM operations return.
//! All ORM methods return `Result<T, OrmadaError>` which can be
//! seamlessly converted to application-level errors using the `?` operator.
//!
//! # Examples
//!
//! ```rust,ignore
//! use ormada::error::OrmadaError;
//!
//! async fn get_book(id: i32) -> Result<BookDTO, AppError> {
//!     let book = Book::objects(db).get(id).await?;  // Auto-converts to AppError
//!     Ok(book.into())
//! }
//! ```

use std::fmt;

/// Error type for all ormada operations
///
/// This enum represents all possible errors that can occur during ORM operations.
/// Uses Rust enums for type-safe, exhaustive error handling.
///
/// # Variants
///
/// - `Database` - Wraps SeaORM database errors
/// - `NotFound` - Record not found by ID
/// - `Validation` - Field validation failed
/// - `EmptyResult` - Query returned no results (first, last, etc.)
/// - `MissingConfiguration` - Builder missing required config
/// - `ConcurrencyConflict` - Race condition after retries
///
/// # Pattern Matching
///
/// ```rust,ignore
/// match Book::objects(db).get(id).await {
///     Ok(book) => println!("Found: {}", book.title),
///     Err(OrmadaError::NotFound { entity, id }) => {
///         eprintln!("{entity} {id} not found");
///     }
///     Err(OrmadaError::Database(e)) => {
///         eprintln!("Database error: {}", e);
///     }
///     Err(e) => eprintln!("Error: {}", e),
/// }
/// ```
///
/// ## Checking error type
///
/// ```rust,ignore
/// if let Err(e) = Book::objects(db).get(999).await {
///     if e.to_string().contains("not found") {
///         println!("Book doesn't exist");
///     }
/// }
/// ```
///
/// # Conversion to Application Errors
///
/// Create a newtype wrapper in your application to convert to your error type:
///
/// ```rust,ignore
/// pub struct AppError(pub OrmadaError);
///
/// impl From<OrmadaError> for AppError {
///     fn from(err: OrmadaError) -> Self {
///         AppError(err)
///     }
/// }
///
/// impl From<AppError> for ServerFnError {
///     fn from(err: AppError) -> Self {
///         ServerFnError::new(err.0.to_string())
///     }
/// }
/// ```
#[derive(Debug)]
pub enum OrmadaError {
    /// Database error from `SeaORM`
    ///
    /// Includes: connection errors, query syntax errors, constraint violations,
    /// transaction errors, and other database-level issues.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Connection error
    /// Err(OrmadaError::Database(DbErr::Conn(...)))
    ///
    /// // Query error
    /// Err(OrmadaError::Database(DbErr::Query(...)))
    ///
    /// // Constraint violation (e.g., duplicate key)
    /// Err(OrmadaError::Database(DbErr::Exec(...)))
    /// ```
    Database(sea_orm::DbErr),

    /// Record not found error
    ///
    /// Returned when a query expects exactly one record but finds none.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// match Book::objects(db).get(id).await {
    ///     Ok(book) => println!("Found: {}", book.title),
    ///     Err(OrmadaError::NotFound { entity, .. }) => {
    ///         println!("No {} found", entity);
    ///     }
    ///     Err(e) => return Err(e),
    /// }
    /// ```
    NotFound {
        /// Entity name (e.g., "Book", "Author")
        entity: &'static str,
        /// Identifier that was searched for
        id: String,
    },

    /// Field validation error
    ///
    /// Raised when model field validation fails (`max_length`, range, etc.)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// match author.save(db).await {
    ///     Ok(_) => println!("Saved"),
    ///     Err(OrmadaError::Validation { field, reason, .. }) => {
    ///         println!("Field '{}' failed: {}", field, reason);
    ///     }
    ///     Err(e) => return Err(e),
    /// }
    /// ```
    Validation {
        /// Entity name
        entity: &'static str,
        /// Field name that failed validation
        field: &'static str,
        /// Reason for validation failure
        reason: String,
    },

    /// Empty result error
    ///
    /// Returned when a query expects at least one record but finds none.
    /// Used for `first()`, `last()`, `earliest()`, `latest()`.
    EmptyResult {
        /// The operation that returned no results
        operation: &'static str,
    },

    /// Missing configuration error
    ///
    /// Returned when a builder method is called without required configuration.
    MissingConfiguration {
        /// The method that failed
        method: &'static str,
        /// The missing configuration
        missing: &'static str,
    },

    /// Concurrency conflict error
    ///
    /// Returned when concurrent operations conflict after retry attempts.
    ConcurrencyConflict {
        /// The operation that failed
        operation: &'static str,
        /// Number of retry attempts made
        attempts: u8,
    },
}

impl fmt::Display for OrmadaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(e) => write!(f, "Database error: {e}"),
            Self::NotFound { entity, id } => {
                write!(f, "{entity} with id '{id}' not found")
            }
            Self::Validation { entity, field, reason } => {
                write!(f, "Validation error in {entity}.{field}: {reason}")
            }
            Self::EmptyResult { operation } => {
                write!(f, "No records found for {operation}")
            }
            Self::MissingConfiguration { method, missing } => {
                write!(f, "{method}: {missing} must be called first")
            }
            Self::ConcurrencyConflict { operation, attempts } => {
                write!(f, "{operation} failed after {attempts} retry attempts due to concurrent operations")
            }
        }
    }
}

impl std::error::Error for OrmadaError {}

impl From<sea_orm::DbErr> for OrmadaError {
    fn from(err: sea_orm::DbErr) -> Self {
        Self::Database(err)
    }
}

impl OrmadaError {
    /// Create a `NotFound` error
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// return Err(OrmadaError::not_found("Book", id.to_string()));
    /// ```
    pub fn not_found(entity: &'static str, id: impl ToString) -> Self {
        Self::NotFound { entity, id: id.to_string() }
    }

    /// Create a Validation error
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// return Err(OrmadaError::validation(
    ///     "Author",
    ///     "name",
    ///     "must not be empty"
    /// ));
    /// ```
    pub fn validation(entity: &'static str, field: &'static str, reason: impl ToString) -> Self {
        Self::Validation {
            entity,
            field,
            reason: reason.to_string(),
        }
    }

    /// Create an EmptyResult error
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// return Err(OrmadaError::empty_result("first"));
    /// ```
    pub const fn empty_result(operation: &'static str) -> Self {
        Self::EmptyResult { operation }
    }

    /// Create a MissingConfiguration error
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// return Err(OrmadaError::missing_config("execute", "on_conflict"));
    /// ```
    pub const fn missing_config(method: &'static str, missing: &'static str) -> Self {
        Self::MissingConfiguration { method, missing }
    }

    /// Create a ConcurrencyConflict error
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// return Err(OrmadaError::concurrency_conflict("get_or_create", 3));
    /// ```
    pub const fn concurrency_conflict(operation: &'static str, attempts: u8) -> Self {
        Self::ConcurrencyConflict { operation, attempts }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::DbErr;

    #[test]
    fn test_database_error_conversion() {
        let db_err = DbErr::RecordNotFound("test".to_string());
        let django_err: OrmadaError = db_err.into();

        match django_err {
            OrmadaError::Database(_) => (),
            _ => panic!("Expected Database variant"),
        }
    }

    #[test]
    fn test_database_error_display() {
        let db_err = DbErr::RecordNotFound("User with id=5".to_string());
        let error = OrmadaError::Database(db_err);
        assert!(error.to_string().contains("Database error"));
        assert!(error.to_string().contains("User with id=5"));
    }

    #[test]
    fn test_error_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<OrmadaError>();
    }

    #[test]
    fn test_error_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<OrmadaError>();
    }

    #[test]
    fn test_error_trait_implementation() {
        let error = OrmadaError::validation("test", "field", "reason");
        let _: &dyn std::error::Error = &error;
    }

    #[test]
    fn test_not_found_error() {
        let error = OrmadaError::not_found("Book", 123);
        assert!(error.to_string().contains("Book"));
        assert!(error.to_string().contains("123"));
        assert!(error.to_string().contains("not found"));
    }

    #[test]
    fn test_not_found_pattern_matching() {
        let error = OrmadaError::NotFound { entity: "Author", id: "456".to_string() };

        match error {
            OrmadaError::NotFound { entity, id } => {
                assert_eq!(entity, "Author");
                assert_eq!(id, "456");
            }
            _ => panic!("Expected NotFound variant"),
        }
    }

    #[test]
    fn test_validation_error() {
        let error = OrmadaError::validation("User", "email", "invalid format");
        assert!(error.to_string().contains("User"));
        assert!(error.to_string().contains("email"));
        assert!(error.to_string().contains("invalid format"));
        assert!(error.to_string().contains("Validation"));
    }

    #[test]
    fn test_validation_pattern_matching() {
        let error = OrmadaError::Validation {
            entity: "Book",
            field: "title",
            reason: "too long".to_string(),
        };

        match error {
            OrmadaError::Validation { entity, field, reason } => {
                assert_eq!(entity, "Book");
                assert_eq!(field, "title");
                assert_eq!(reason, "too long");
            }
            _ => panic!("Expected Validation variant"),
        }
    }

    #[test]
    fn test_error_variants_are_distinct() {
        let not_found = OrmadaError::not_found("Book", 1);
        let validation = OrmadaError::validation("Book", "title", "required");
        let empty = OrmadaError::empty_result("first");

        assert!(matches!(not_found, OrmadaError::NotFound { .. }));
        assert!(matches!(validation, OrmadaError::Validation { .. }));
        assert!(matches!(empty, OrmadaError::EmptyResult { .. }));
    }

    #[test]
    fn test_empty_result_error() {
        let error = OrmadaError::empty_result("first");
        assert!(error.to_string().contains("No records found"));
        assert!(error.to_string().contains("first"));
    }

    #[test]
    fn test_missing_config_error() {
        let error = OrmadaError::missing_config("execute", "on_conflict");
        assert!(error.to_string().contains("execute"));
        assert!(error.to_string().contains("on_conflict"));
    }

    #[test]
    fn test_concurrency_conflict_error() {
        let error = OrmadaError::concurrency_conflict("get_or_create", 3);
        assert!(error.to_string().contains("get_or_create"));
        assert!(error.to_string().contains("3"));
        assert!(error.to_string().contains("retry"));
    }
}
