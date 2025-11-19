//! Error types for seaorm-django
//!
//! This module provides the error type that all ORM operations return.
//! All ORM methods return `Result<T, DjangoOrmError>` which can be
//! seamlessly converted to application-level errors using the `?` operator.
//!
//! # Examples
//!
//! ```rust,ignore
//! use seaorm_django::error::DjangoOrmError;
//!
//! async fn get_book(id: i32) -> Result<BookDTO, AppError> {
//!     let book = Book::objects(db).get(id).await?;  // Auto-converts to AppError
//!     Ok(book.into())
//! }
//! ```

use std::fmt;

/// Error type for all seaorm-django operations
///
/// This enum represents all possible errors that can occur during ORM operations.
/// It wraps SeaORM's database errors and provides custom error messages.
///
/// # Variants
///
/// - `Database(DbErr)` - A database operation failed (connection, query, constraint, etc.)
/// - `Custom(String)` - A custom error message (e.g., "Record not found")
///
/// # Error Handling
///
/// ## Using the ? operator
///
/// ```rust,ignore
/// // Errors propagate automatically
/// let book = Book::objects(db).get(1).await?;  // Propagates if not found or DB error
/// ```
///
/// ## Pattern matching
///
/// ```rust,ignore
/// match Book::objects(db).get(id).await {
///     Ok(book) => println!("Found: {}", book.title),
///     Err(DjangoOrmError::Database(e)) => {
///         eprintln!("Database error: {}", e);
///     }
///     Err(DjangoOrmError::Custom(msg)) => {
///         eprintln!("Custom error: {}", msg);
///     }
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
/// pub struct AppError(pub DjangoOrmError);
///
/// impl From<DjangoOrmError> for AppError {
///     fn from(err: DjangoOrmError) -> Self {
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
pub enum DjangoOrmError {
    /// Database error from SeaORM
    ///
    /// Includes: connection errors, query syntax errors, constraint violations,
    /// transaction errors, and other database-level issues.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Connection error
    /// Err(DjangoOrmError::Database(DbErr::Conn(...)))
    ///
    /// // Query error
    /// Err(DjangoOrmError::Database(DbErr::Query(...)))
    ///
    /// // Constraint violation (e.g., duplicate key)
    /// Err(DjangoOrmError::Database(DbErr::Exec(...)))
    /// ```
    Database(sea_orm::DbErr),

    /// Custom error message
    ///
    /// Used for application-level errors like "Record not found",
    /// "Invalid state", etc.
    ///
    Custom(String),
    
    /// Field validation error
    ///
    /// Raised when model field validation fails (max_length, range, etc.)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // String too long
    /// Err(DjangoOrmError::ValidationError("Field 'username' exceeds max_length of 50"))
    ///
    /// // Number out of range
    /// Err(DjangoOrmError::ValidationError("Field 'age' value 15 is less than minimum 18"))
    /// ```
    ValidationError(String),
}

impl fmt::Display for DjangoOrmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(e) => write!(f, "Database error: {}", e),
            Self::Custom(msg) => write!(f, "{}", msg),
            Self::ValidationError(msg) => write!(f, "Validation error: {}", msg),
        }
    }
}

impl std::error::Error for DjangoOrmError {}

impl From<sea_orm::DbErr> for DjangoOrmError {
    fn from(err: sea_orm::DbErr) -> Self {
        Self::Database(err)
    }
}

impl From<String> for DjangoOrmError {
    fn from(msg: String) -> Self {
        DjangoOrmError::Custom(msg)
    }
}

impl From<&str> for DjangoOrmError {
    fn from(msg: &str) -> Self {
        DjangoOrmError::Custom(msg.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::DbErr;

    #[test]
    fn test_custom_error_display() {
        let error = DjangoOrmError::Custom("Test error message".to_string());
        assert_eq!(error.to_string(), "Test error message");
    }

    #[test]
    fn test_custom_error_with_empty_string() {
        let error = DjangoOrmError::Custom(String::new());
        assert_eq!(error.to_string(), "");
    }

    #[test]
    fn test_custom_error_from_string() {
        let error: DjangoOrmError = "Test error".to_string().into();
        assert_eq!(error.to_string(), "Test error");
    }

    #[test]
    fn test_custom_error_from_str() {
        let error: DjangoOrmError = "Test error".into();
        assert_eq!(error.to_string(), "Test error");
    }

    #[test]
    fn test_database_error_conversion() {
        let db_err = DbErr::RecordNotFound("test".to_string());
        let django_err: DjangoOrmError = db_err.into();

        match django_err {
            DjangoOrmError::Database(_) => (),
            _ => panic!("Expected Database variant"),
        }
    }

    #[test]
    fn test_database_error_display() {
        let db_err = DbErr::RecordNotFound("User with id=5".to_string());
        let error = DjangoOrmError::Database(db_err);
        assert!(error.to_string().contains("Database error"));
        assert!(error.to_string().contains("User with id=5"));
    }

    #[test]
    fn test_error_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<DjangoOrmError>();
    }

    #[test]
    fn test_error_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<DjangoOrmError>();
    }

    #[test]
    fn test_error_trait_implementation() {
        let error = DjangoOrmError::Custom("test".to_string());
        let _: &dyn std::error::Error = &error;
    }

    #[test]
    fn test_custom_error_with_special_characters() {
        let error = DjangoOrmError::Custom("Error with 'quotes' and \"double quotes\"".to_string());
        assert!(error.to_string().contains("'quotes'"));
        assert!(error.to_string().contains("\"double quotes\""));
    }

    #[test]
    fn test_custom_error_with_unicode() {
        let error = DjangoOrmError::Custom("Error with emoji 🚀 and unicode ñ".to_string());
        assert!(error.to_string().contains("🚀"));
        assert!(error.to_string().contains("ñ"));
    }
}
