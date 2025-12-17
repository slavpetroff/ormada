//! Error types for ormada - Django-style naming
//!
//! This module provides comprehensive error types for all ORM operations,
//! using Django-style naming conventions for familiarity.
//!
//! # Django-style Error Names
//!
//! | Ormada | Django Equivalent |
//! |--------|-------------------|
//! | `DoesNotExist` | `Model.DoesNotExist` |
//! | `MultipleObjectsReturned` | `Model.MultipleObjectsReturned` |
//! | `IntegrityError` | `django.db.IntegrityError` |
//! | `ValidationError` | `django.core.exceptions.ValidationError` |
//! | `OperationalError` | `django.db.OperationalError` |
//! | `ProgrammingError` | `django.db.ProgrammingError` |
//!
//! # Creating Errors
//!
//! ```rust,ignore
//! // Record not found (Django: Model.DoesNotExist)
//! OrmadaError::DoesNotExist { entity: "Book", id: "123".into() }
//!
//! // Constraint violation (Django: IntegrityError)
//! OrmadaError::IntegrityError("Duplicate entry for key 'email'".into())
//!
//! // Validation failure (Django: ValidationError)
//! OrmadaError::ValidationError { entity: "Author", field: "name", reason: "too long".into() }
//!
//! // Connection issues (Django: OperationalError)
//! OrmadaError::OperationalError("Connection refused".into())
//!
//! // SQL syntax errors (Django: ProgrammingError)
//! OrmadaError::ProgrammingError("Unknown column 'foo'".into())
//! ```
//!
//! # Pattern Matching
//!
//! ```rust,ignore
//! match Book::objects(db).get(id).await {
//!     Ok(book) => println!("Found: {}", book.title),
//!     Err(OrmadaError::DoesNotExist { entity, id }) => {
//!         // Like Django's: except Book.DoesNotExist
//!         eprintln!("{entity} with id '{id}' not found");
//!     }
//!     Err(OrmadaError::IntegrityError(msg)) => {
//!         // Like Django's: except IntegrityError
//!         eprintln!("Integrity error: {msg}");
//!     }
//!     Err(e) => eprintln!("Error: {e}"),
//! }
//! ```

use std::fmt;

/// Error type for all ormada operations (Django-style naming)
///
/// Comprehensive error enum using Django-style naming conventions.
/// Each variant maps to a Django exception for familiarity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrmadaError {
    // =========================================================================
    // Django: OperationalError - Connection & operational issues
    // =========================================================================
    /// Database operational error (Django: `OperationalError`)
    ///
    /// Raised for connection issues, timeouts, and other operational problems.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// OrmadaError::OperationalError("Connection refused".into())
    /// OrmadaError::OperationalError("Connection pool exhausted".into())
    /// OrmadaError::OperationalError("Lock wait timeout exceeded".into())
    /// ```
    OperationalError(String),

    // =========================================================================
    // Django: ProgrammingError - SQL/Query errors
    // =========================================================================
    /// SQL programming error (Django: `ProgrammingError`)
    ///
    /// Raised for SQL syntax errors, unknown columns, invalid queries.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// OrmadaError::ProgrammingError("Unknown column 'foo'".into())
    /// OrmadaError::ProgrammingError("Table 'users' doesn't exist".into())
    /// ```
    ProgrammingError(String),

    // =========================================================================
    // Django: IntegrityError - Constraint violations
    // =========================================================================
    /// Database integrity error (Django: `IntegrityError`)
    ///
    /// Raised for constraint violations: unique, foreign key, check constraints.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Duplicate key
    /// OrmadaError::IntegrityError("Duplicate entry for key 'email'".into())
    ///
    /// // Foreign key violation
    /// OrmadaError::IntegrityError("Foreign key constraint failed".into())
    ///
    /// // Check constraint
    /// OrmadaError::IntegrityError("Check constraint 'age_positive' violated".into())
    /// ```
    IntegrityError(String),

    // =========================================================================
    // Django: Model.DoesNotExist - Record not found
    // =========================================================================
    /// Record does not exist (Django: `Model.DoesNotExist`)
    ///
    /// Raised when `get()` finds no matching record.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Like Django's: Book.objects.get(id=123) raising Book.DoesNotExist
    /// OrmadaError::DoesNotExist { entity: "Book", id: "123".into() }
    /// ```
    DoesNotExist {
        /// Entity/Model name (e.g., "Book", "Author")
        entity: &'static str,
        /// Identifier that was searched for
        id: String,
    },

    /// Empty query result (variant of DoesNotExist)
    ///
    /// Raised when `first()`, `last()`, etc. find no records.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// OrmadaError::EmptyResultSet { operation: "first" }
    /// ```
    EmptyResultSet {
        /// The operation that returned no results
        operation: &'static str,
    },

    // =========================================================================
    // Django: Model.MultipleObjectsReturned
    // =========================================================================
    /// Multiple objects returned (Django: `Model.MultipleObjectsReturned`)
    ///
    /// Raised when `get()` finds more than one matching record.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// OrmadaError::MultipleObjectsReturned { entity: "Book", count: 3 }
    /// ```
    MultipleObjectsReturned {
        /// Entity/Model name
        entity: &'static str,
        /// Number of objects found
        count: usize,
    },

    // =========================================================================
    // Django: ValidationError - Field validation
    // =========================================================================
    /// Field validation error (Django: `ValidationError`)
    ///
    /// Raised when model field validation fails.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// OrmadaError::ValidationError {
    ///     entity: "Author",
    ///     field: "name",
    ///     reason: "exceeds max length of 100".into()
    /// }
    /// ```
    ValidationError {
        /// Entity/Model name
        entity: &'static str,
        /// Field name that failed validation
        field: &'static str,
        /// Reason for validation failure
        reason: String,
    },

    // =========================================================================
    // Django: DataError - Type conversion issues
    // =========================================================================
    /// Data/type conversion error (Django: `DataError`)
    ///
    /// Raised when converting between Rust types and database types fails.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// OrmadaError::DataError {
    ///     from: "String",
    ///     to: "i32",
    ///     reason: "invalid digit found".into()
    /// }
    /// ```
    DataError {
        /// Source type name
        from: &'static str,
        /// Target type name
        to: &'static str,
        /// Conversion failure reason
        reason: String,
    },

    // =========================================================================
    // Django: TransactionManagementError
    // =========================================================================
    /// Transaction error (Django: `TransactionManagementError`)
    ///
    /// Raised for transaction-specific failures.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// OrmadaError::TransactionError("Deadlock detected".into())
    /// OrmadaError::TransactionError("Transaction rolled back".into())
    /// ```
    TransactionError(String),

    // =========================================================================
    // Migration errors
    // =========================================================================
    /// Migration error
    ///
    /// Raised when database migrations fail.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// OrmadaError::MigrationError("Migration '20231201_create_users' failed".into())
    /// ```
    MigrationError(String),

    // =========================================================================
    // Record operation errors
    // =========================================================================
    /// Record not inserted
    ///
    /// Raised when insert fails (e.g., all records conflicted on upsert).
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// OrmadaError::RecordNotInserted("All records conflicted".into())
    /// ```
    RecordNotInserted(String),

    /// Record not updated
    ///
    /// Raised when update matches no rows.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// OrmadaError::RecordNotUpdated("No matching records".into())
    /// ```
    RecordNotUpdated(String),

    /// Concurrency conflict
    ///
    /// Raised after retry attempts fail due to concurrent modifications.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// OrmadaError::ConcurrencyError { operation: "get_or_create", attempts: 3 }
    /// ```
    ConcurrencyError {
        /// The operation that failed
        operation: &'static str,
        /// Number of retry attempts made
        attempts: u8,
    },

    // =========================================================================
    // Configuration errors
    // =========================================================================
    /// Configuration error (Django: `ImproperlyConfigured`)
    ///
    /// Raised for missing or invalid configuration.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// OrmadaError::ConfigurationError("DATABASE_URL not set".into())
    /// ```
    ConfigurationError(String),
}

impl fmt::Display for OrmadaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Django: OperationalError
            Self::OperationalError(msg) => write!(f, "OperationalError: {msg}"),

            // Django: ProgrammingError
            Self::ProgrammingError(msg) => write!(f, "ProgrammingError: {msg}"),

            // Django: IntegrityError
            Self::IntegrityError(msg) => write!(f, "IntegrityError: {msg}"),

            // Django: DoesNotExist
            Self::DoesNotExist { entity, id } => {
                write!(f, "{entity}.DoesNotExist: {entity} matching query does not exist (id={id})")
            }
            Self::EmptyResultSet { operation } => {
                write!(f, "DoesNotExist: {operation}() returned no results")
            }

            // Django: MultipleObjectsReturned
            Self::MultipleObjectsReturned { entity, count } => {
                write!(f, "{entity}.MultipleObjectsReturned: get() returned {count} objects")
            }

            // Django: ValidationError
            Self::ValidationError { entity, field, reason } => {
                write!(f, "ValidationError: {entity}.{field}: {reason}")
            }

            // Django: DataError
            Self::DataError { from, to, reason } => {
                write!(f, "DataError: cannot convert {from} to {to}: {reason}")
            }

            // Django: TransactionManagementError
            Self::TransactionError(msg) => write!(f, "TransactionError: {msg}"),

            // Migration
            Self::MigrationError(msg) => write!(f, "MigrationError: {msg}"),

            // Record operations
            Self::RecordNotInserted(msg) => write!(f, "RecordNotInserted: {msg}"),
            Self::RecordNotUpdated(msg) => write!(f, "RecordNotUpdated: {msg}"),
            Self::ConcurrencyError { operation, attempts } => {
                write!(f, "ConcurrencyError: {operation} failed after {attempts} attempts")
            }

            // Django: ImproperlyConfigured
            Self::ConfigurationError(msg) => write!(f, "ConfigurationError: {msg}"),
        }
    }
}

impl std::error::Error for OrmadaError {}

impl From<sea_orm::DbErr> for OrmadaError {
    fn from(err: sea_orm::DbErr) -> Self {
        use sea_orm::DbErr;
        match &err {
            // Connection/operational errors -> OperationalError
            DbErr::Conn(_) | DbErr::ConnectionAcquire(_) => Self::OperationalError(err.to_string()),
            // Query/syntax errors -> ProgrammingError
            DbErr::Query(_) => Self::ProgrammingError(err.to_string()),
            // Execution errors -> could be IntegrityError or OperationalError
            DbErr::Exec(_) => Self::IntegrityError(err.to_string()),
            // Record not found -> DoesNotExist
            DbErr::RecordNotFound(msg) => Self::DoesNotExist { entity: "Record", id: msg.clone() },
            // Type conversion errors -> DataError
            DbErr::Type(msg) | DbErr::Json(msg) => Self::DataError {
                from: "database",
                to: "rust",
                reason: msg.clone(),
            },
            DbErr::TryIntoErr { from, into, .. } => {
                Self::DataError { from, to: into, reason: err.to_string() }
            }
            // Migration errors
            DbErr::Migration(msg) => Self::MigrationError(msg.clone()),
            // Insert/Update failures
            DbErr::RecordNotInserted => Self::RecordNotInserted("No records were inserted".into()),
            DbErr::RecordNotUpdated => Self::RecordNotUpdated("No records were updated".into()),
            // Custom errors -> ProgrammingError
            DbErr::Custom(msg) => Self::ProgrammingError(msg.clone()),
            // Catch-all -> OperationalError
            _ => Self::OperationalError(err.to_string()),
        }
    }
}

impl OrmadaError {
    // =========================================================================
    // Constructors (Django-style convenience methods)
    // =========================================================================

    /// Create a `DoesNotExist` error (Django: `Model.DoesNotExist`)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // When a record doesn't exist
    /// return Err(OrmadaError::does_not_exist("Book", 123));
    ///
    /// // In a service function
    /// let book = Book::objects(db).get(id).await.map_err(|_| {
    ///     OrmadaError::does_not_exist("Book", id)
    /// })?;
    /// ```
    pub fn does_not_exist(entity: &'static str, id: impl ToString) -> Self {
        Self::DoesNotExist { entity, id: id.to_string() }
    }

    /// Create a `MultipleObjectsReturned` error (Django: `Model.MultipleObjectsReturned`)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// return Err(OrmadaError::multiple_objects_returned("Book", 3));
    /// ```
    pub const fn multiple_objects_returned(entity: &'static str, count: usize) -> Self {
        Self::MultipleObjectsReturned { entity, count }
    }

    /// Create a `ValidationError` (Django: `ValidationError`)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Single field validation
    /// return Err(OrmadaError::validation_error("Author", "email", "invalid format"));
    ///
    /// // In model validation
    /// if name.len() > 100 {
    ///     return Err(OrmadaError::validation_error("Author", "name", "exceeds 100 chars"));
    /// }
    /// ```
    pub fn validation_error(
        entity: &'static str,
        field: &'static str,
        reason: impl ToString,
    ) -> Self {
        Self::ValidationError {
            entity,
            field,
            reason: reason.to_string(),
        }
    }

    /// Create a `DataError` (Django: `DataError`)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// return Err(OrmadaError::data_error("String", "i32", "invalid digit"));
    /// ```
    pub fn data_error(from: &'static str, to: &'static str, reason: impl ToString) -> Self {
        Self::DataError { from, to, reason: reason.to_string() }
    }

    /// Create an `EmptyResultSet` error
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// return Err(OrmadaError::empty_result_set("first"));
    /// ```
    pub const fn empty_result_set(operation: &'static str) -> Self {
        Self::EmptyResultSet { operation }
    }

    /// Create a `ConcurrencyError`
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// return Err(OrmadaError::concurrency_error("get_or_create", 3));
    /// ```
    pub const fn concurrency_error(operation: &'static str, attempts: u8) -> Self {
        Self::ConcurrencyError { operation, attempts }
    }

    // =========================================================================
    // Django-style error type checking
    // =========================================================================

    /// Check if this is a `DoesNotExist` error (Django: `ObjectDoesNotExist`)
    ///
    /// Returns `true` for `DoesNotExist` and `EmptyResultSet` variants.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Similar to Django's: except ObjectDoesNotExist
    /// match Book::objects(db).get(id).await {
    ///     Ok(book) => Ok(book),
    ///     Err(e) if e.is_does_not_exist() => {
    ///         // Handle missing record
    ///         Err(e)
    ///     }
    ///     Err(e) => Err(e),
    /// }
    /// ```
    pub fn is_does_not_exist(&self) -> bool {
        matches!(self, Self::DoesNotExist { .. } | Self::EmptyResultSet { .. })
    }

    /// Check if this is an `IntegrityError` (Django: `IntegrityError`)
    ///
    /// Returns `true` for constraint violations like duplicate keys, FK violations.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Similar to Django's: except IntegrityError
    /// match Author::objects(db).create(author).await {
    ///     Ok(a) => Ok(a),
    ///     Err(e) if e.is_integrity_error() => {
    ///         // Handle duplicate email, FK violation, etc.
    ///         Err(e)
    ///     }
    ///     Err(e) => Err(e),
    /// }
    /// ```
    pub fn is_integrity_error(&self) -> bool {
        matches!(self, Self::IntegrityError(_))
    }

    /// Check if this is a `ValidationError` (Django: `ValidationError`)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Similar to Django's: except ValidationError
    /// if err.is_validation_error() {
    ///     // Show field-specific error messages to user
    /// }
    /// ```
    pub fn is_validation_error(&self) -> bool {
        matches!(self, Self::ValidationError { .. })
    }

    /// Check if this is an `OperationalError` (Django: `OperationalError`)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// if err.is_operational_error() {
    ///     // Connection issue, timeout, etc.
    /// }
    /// ```
    pub fn is_operational_error(&self) -> bool {
        matches!(self, Self::OperationalError(_))
    }

    /// Check if this is a `ProgrammingError` (Django: `ProgrammingError`)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// if err.is_programming_error() {
    ///     // SQL syntax error, unknown column, etc.
    /// }
    /// ```
    pub fn is_programming_error(&self) -> bool {
        matches!(self, Self::ProgrammingError(_))
    }

    /// Check if this is a `TransactionError`
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// if err.is_transaction_error() {
    ///     // Transaction was rolled back, may need to retry
    /// }
    /// ```
    pub fn is_transaction_error(&self) -> bool {
        matches!(self, Self::TransactionError(_))
    }

    /// Check if this error is retryable
    ///
    /// Returns `true` for transient errors that might succeed on retry:
    /// - OperationalError (connection might recover)
    /// - ConcurrencyError (might succeed after other transaction commits)
    /// - TransactionError (deadlocks might resolve)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// async fn with_retry<F, T>(mut f: F) -> Result<T, OrmadaError>
    /// where
    ///     F: FnMut() -> Future<Output = Result<T, OrmadaError>>
    /// {
    ///     for _ in 0..3 {
    ///         match f().await {
    ///             Ok(v) => return Ok(v),
    ///             Err(e) if e.is_retryable() => continue,
    ///             Err(e) => return Err(e),
    ///         }
    ///     }
    ///     Err(OrmadaError::OperationalError("Max retries exceeded".into()))
    /// }
    /// ```
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::OperationalError(_) | Self::ConcurrencyError { .. } | Self::TransactionError(_)
        )
    }

    // =========================================================================
    // Error inspection (get details from errors)
    // =========================================================================

    /// Get the entity name if this is a `DoesNotExist`, `MultipleObjectsReturned`, or `ValidationError`
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// if let Some(entity) = err.entity() {
    ///     println!("{} error occurred", entity);
    /// }
    /// ```
    pub fn entity(&self) -> Option<&'static str> {
        match self {
            Self::DoesNotExist { entity, .. } => Some(entity),
            Self::MultipleObjectsReturned { entity, .. } => Some(entity),
            Self::ValidationError { entity, .. } => Some(entity),
            _ => None,
        }
    }

    /// Get the field name if this is a `ValidationError`
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// if let Some(field) = err.field() {
    ///     println!("Error in field: {}", field);
    /// }
    /// ```
    pub fn field(&self) -> Option<&'static str> {
        match self {
            Self::ValidationError { field, .. } => Some(field),
            _ => None,
        }
    }

    /// Get the error message
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// println!("Error: {}", err.message());
    /// ```
    pub fn message(&self) -> String {
        self.to_string()
    }

    // =========================================================================
    // Specific error type checks
    // =========================================================================

    /// Check if this is a duplicate entry error (subset of IntegrityError)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// if err.is_duplicate_entry() {
    ///     return Err(OrmadaError::ValidationError {
    ///         entity: "User",
    ///         field: "email",
    ///         reason: "Email already exists".into()
    ///     });
    /// }
    /// ```
    pub fn is_duplicate_entry(&self) -> bool {
        match self {
            Self::IntegrityError(msg) => {
                let lower = msg.to_lowercase();
                lower.contains("duplicate")
                    || lower.contains("unique")
                    || lower.contains("already exists")
            }
            _ => false,
        }
    }

    /// Check if this is a foreign key violation (subset of IntegrityError)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// if err.is_foreign_key_violation() {
    ///     return Err(OrmadaError::DoesNotExist {
    ///         entity: "Author",
    ///         id: author_id.to_string()
    ///     });
    /// }
    /// ```
    pub fn is_foreign_key_violation(&self) -> bool {
        match self {
            Self::IntegrityError(msg) => {
                let lower = msg.to_lowercase();
                lower.contains("foreign key")
                    || lower.contains("fk_")
                    || lower.contains("references")
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::DbErr;

    #[test]
    fn test_database_error_conversion_record_not_found() {
        let db_err = DbErr::RecordNotFound("test".to_string());
        let ormada_err: OrmadaError = db_err.into();

        match ormada_err {
            OrmadaError::DoesNotExist { .. } => (),
            _ => panic!("Expected DoesNotExist variant for RecordNotFound"),
        }
    }

    #[test]
    fn test_database_error_conversion_connection() {
        let db_err = DbErr::Conn(sea_orm::RuntimeErr::Internal("connection failed".to_string()));
        let ormada_err: OrmadaError = db_err.into();

        match ormada_err {
            OrmadaError::OperationalError(msg) => {
                assert!(msg.contains("connection"));
            }
            _ => panic!("Expected OperationalError variant"),
        }
    }

    #[test]
    fn test_operational_error() {
        let error = OrmadaError::OperationalError("Database not initialized".into());
        assert!(error.to_string().contains("OperationalError"));
        assert!(error.to_string().contains("Database not initialized"));
    }

    #[test]
    fn test_programming_error() {
        let error = OrmadaError::ProgrammingError("Invalid SQL syntax".into());
        assert!(error.to_string().contains("ProgrammingError"));
        assert!(error.to_string().contains("Invalid SQL syntax"));
    }

    #[test]
    fn test_integrity_error() {
        let error = OrmadaError::IntegrityError("Duplicate key violation".into());
        assert!(error.to_string().contains("IntegrityError"));
        assert!(error.to_string().contains("Duplicate key"));
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
        let error = OrmadaError::validation_error("test", "field", "reason");
        let _: &dyn std::error::Error = &error;
    }

    #[test]
    fn test_does_not_exist_error() {
        let error = OrmadaError::does_not_exist("Book", 123);
        assert!(error.to_string().contains("Book"));
        assert!(error.to_string().contains("123"));
        assert!(error.to_string().contains("DoesNotExist"));
    }

    #[test]
    fn test_does_not_exist_pattern_matching() {
        let error = OrmadaError::DoesNotExist { entity: "Author", id: "456".to_string() };

        match error {
            OrmadaError::DoesNotExist { entity, id } => {
                assert_eq!(entity, "Author");
                assert_eq!(id, "456");
            }
            _ => panic!("Expected DoesNotExist variant"),
        }
    }

    #[test]
    fn test_multiple_objects_returned() {
        let error = OrmadaError::multiple_objects_returned("Book", 3);
        assert!(error.to_string().contains("Book"));
        assert!(error.to_string().contains("3"));
        assert!(error.to_string().contains("MultipleObjectsReturned"));
    }

    #[test]
    fn test_validation_error() {
        let error = OrmadaError::validation_error("User", "email", "invalid format");
        assert!(error.to_string().contains("User"));
        assert!(error.to_string().contains("email"));
        assert!(error.to_string().contains("invalid format"));
        assert!(error.to_string().contains("ValidationError"));
    }

    #[test]
    fn test_validation_pattern_matching() {
        let error = OrmadaError::ValidationError {
            entity: "Book",
            field: "title",
            reason: "too long".to_string(),
        };

        match error {
            OrmadaError::ValidationError { entity, field, reason } => {
                assert_eq!(entity, "Book");
                assert_eq!(field, "title");
                assert_eq!(reason, "too long");
            }
            _ => panic!("Expected ValidationError variant"),
        }
    }

    #[test]
    fn test_error_variants_are_distinct() {
        let does_not_exist = OrmadaError::does_not_exist("Book", 1);
        let validation = OrmadaError::validation_error("Book", "title", "required");
        let empty = OrmadaError::empty_result_set("first");

        assert!(matches!(does_not_exist, OrmadaError::DoesNotExist { .. }));
        assert!(matches!(validation, OrmadaError::ValidationError { .. }));
        assert!(matches!(empty, OrmadaError::EmptyResultSet { .. }));
    }

    #[test]
    fn test_empty_result_set_error() {
        let error = OrmadaError::empty_result_set("first");
        assert!(error.to_string().contains("DoesNotExist"));
        assert!(error.to_string().contains("first"));
    }

    #[test]
    fn test_concurrency_error() {
        let error = OrmadaError::concurrency_error("get_or_create", 3);
        assert!(error.to_string().contains("get_or_create"));
        assert!(error.to_string().contains("3"));
        assert!(error.to_string().contains("ConcurrencyError"));
    }

    #[test]
    fn test_data_error() {
        let error = OrmadaError::DataError {
            from: "String",
            to: "i32",
            reason: "invalid digit".into(),
        };
        assert!(error.to_string().contains("DataError"));
        assert!(error.to_string().contains("String"));
        assert!(error.to_string().contains("i32"));
    }

    #[test]
    fn test_transaction_error() {
        let error = OrmadaError::TransactionError("Deadlock detected".into());
        assert!(error.to_string().contains("TransactionError"));
        assert!(error.to_string().contains("Deadlock"));
    }

    #[test]
    fn test_migration_error() {
        let error = OrmadaError::MigrationError("Migration 003 failed".into());
        assert!(error.to_string().contains("MigrationError"));
        assert!(error.to_string().contains("003"));
    }

    #[test]
    fn test_configuration_error() {
        let error = OrmadaError::ConfigurationError("DATABASE_URL not set".into());
        assert!(error.to_string().contains("ConfigurationError"));
        assert!(error.to_string().contains("DATABASE_URL"));
    }

    #[test]
    fn test_record_not_inserted() {
        let error = OrmadaError::RecordNotInserted("All conflicted".into());
        assert!(error.to_string().contains("RecordNotInserted"));
    }

    #[test]
    fn test_record_not_updated() {
        let error = OrmadaError::RecordNotUpdated("No matching rows".into());
        assert!(error.to_string().contains("RecordNotUpdated"));
    }

    #[test]
    fn test_helper_methods() {
        let conn_err = OrmadaError::OperationalError("test".into());
        assert!(conn_err.is_operational_error());
        assert!(conn_err.is_retryable());
        assert!(!conn_err.is_does_not_exist());

        let does_not_exist = OrmadaError::does_not_exist("Book", 1);
        assert!(does_not_exist.is_does_not_exist());
        assert!(!does_not_exist.is_operational_error());

        let integrity = OrmadaError::IntegrityError("duplicate".into());
        assert!(integrity.is_integrity_error());

        let validation = OrmadaError::validation_error("User", "email", "invalid");
        assert!(validation.is_validation_error());
    }

    #[test]
    fn test_django_style_does_not_exist() {
        // DoesNotExist variant
        let does_not_exist = OrmadaError::DoesNotExist { entity: "Book", id: "123".into() };
        assert!(does_not_exist.is_does_not_exist());

        // EmptyResultSet is also DoesNotExist
        let empty = OrmadaError::EmptyResultSet { operation: "first" };
        assert!(empty.is_does_not_exist());

        // Other errors are not DoesNotExist
        let conn = OrmadaError::OperationalError("test".into());
        assert!(!conn.is_does_not_exist());
    }

    #[test]
    fn test_django_style_integrity_error() {
        let integrity = OrmadaError::IntegrityError("UNIQUE constraint failed".into());
        assert!(integrity.is_integrity_error());
        assert!(integrity.is_duplicate_entry());
        assert!(!integrity.is_foreign_key_violation());

        let fk_error = OrmadaError::IntegrityError("FOREIGN KEY constraint failed".into());
        assert!(fk_error.is_integrity_error());
        assert!(fk_error.is_foreign_key_violation());
        assert!(!fk_error.is_duplicate_entry());
    }

    #[test]
    fn test_error_inspection() {
        let does_not_exist = OrmadaError::does_not_exist("Book", 123);
        assert_eq!(does_not_exist.entity(), Some("Book"));
        assert_eq!(does_not_exist.field(), None);

        let validation = OrmadaError::validation_error("Author", "email", "invalid");
        assert_eq!(validation.entity(), Some("Author"));
        assert_eq!(validation.field(), Some("email"));

        let conn = OrmadaError::OperationalError("test".into());
        assert_eq!(conn.entity(), None);
        assert_eq!(conn.field(), None);
    }

    #[test]
    fn test_transaction_retryable() {
        let tx_err = OrmadaError::TransactionError("Deadlock".into());
        assert!(tx_err.is_transaction_error());
        assert!(tx_err.is_retryable());
    }

    #[test]
    fn test_error_equality() {
        let err1 = OrmadaError::OperationalError("test".into());
        let err2 = OrmadaError::OperationalError("test".into());
        let err3 = OrmadaError::OperationalError("other".into());

        assert_eq!(err1, err2);
        assert_ne!(err1, err3);
    }
}
