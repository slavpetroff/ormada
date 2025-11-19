//! Transaction support for atomic database operations (Django's transaction.atomic)
//!
//! This module provides Django-style transaction management with Rust idioms.
//! All operations within a transaction are atomic - they either all succeed or all fail.
//!
//! # Examples
//!
//! ## Basic Transaction
//!
//! ```rust,ignore
//! use seaorm_django::{prelude::*, tx};
//!
//! // All operations succeed or all rollback - clean and simple!
//! tx!(db, |txn| async move {
//!     // Create author
//!     let author = author::Entity::objects(txn).create(author::Model {
//!         name: "John Doe".to_string(),
//!         email: "john@example.com".to_string(),
//!         age: 30,
//!         ..Default::default()
//!     }).await?;
//!     
//!     // Create book referencing the author
//!     let book = book::Entity::objects(txn).create(book::Model {
//!         title: "Rust Book".to_string(),
//!         author_id: author.id,
//!         price: 2999,
//!         ..Default::default()
//!     }).await?;
//!     
//!     Ok((author, book))
//! }).await?;
//! ```
//!
//! ## Error Handling (Automatic Rollback)
//!
//! ```rust,ignore
//! use seaorm_django::tx;
//! 
//! let result = tx!(db, |txn| async move {
//!     let author = create_author(txn).await?;
//!     
//!     // This fails - entire transaction rolls back
//!     if author.age < 18 {
//!         return Err(DjangoOrmError::Custom("Author must be 18+".into()));
//!     }
//!     
//!     let book = create_book(txn, author.id).await?;
//!     Ok(book)
//! }).await;
//! 
//! match result {
//!     Ok(book) => println!("Transaction committed: {}", book.title),
//!     Err(e) => println!("Transaction rolled back: {}", e),
//! }
//! ```
//!
//! ## Nested Transactions (Savepoints)
//!
//! ```rust,ignore
//! use seaorm_django::tx;
//! 
//! tx!(db, |txn| async move {
//!     let author = create_author(txn).await?;
//!     
//!     // Inner transaction with savepoint - nested tx! just works!
//!     let books_result = tx!(txn, |inner_txn| async move {
//!         create_book(inner_txn, author.id, "Book 1").await?;
//!         create_book(inner_txn, author.id, "Book 2").await?;
//!         Ok(2)
//!     }).await;
//!     
//!     // If books fail, only that part rolls back
//!     // Author is still created
//!     let book_count = books_result.unwrap_or(0);
//!     
//!     Ok((author, book_count))
//! }).await?;
//! ```

use crate::error::DjangoOrmError;
use sea_orm::{DatabaseConnection, DatabaseTransaction, TransactionTrait};
use std::future::Future;

/// Extension trait for atomic database operations
///
/// Provides Django-style `transaction.atomic()` functionality with Rust idioms.
/// All operations within a transaction are ACID-compliant.
///
/// **Ergonomic Alternative**: Use the `tx!` macro for cleaner syntax without `Box::pin`:
///
/// ```rust,ignore
/// use seaorm_django::tx;
///
/// tx!(db, |txn| async move {
///     // Your transaction code here
///     Ok(result)
/// }).await?;
/// ```
pub trait AtomicExt {
    /// Execute a closure within a database transaction (Django's transaction.atomic)
    ///
    /// **Tip**: Use the `tx!` macro for cleaner syntax - see examples below.
    ///
    /// All database operations within the closure either succeed (commit) or
    /// fail (rollback) together. This ensures data consistency.
    ///
    /// # Arguments
    ///
    /// * `f` - Async closure containing database operations
    ///
    /// # Returns
    ///
    /// - `Ok(T)` - Transaction committed successfully, returns closure result
    /// - `Err(DjangoOrmError)` - Transaction rolled back due to error
    ///
    /// # Examples
    ///
    /// ## Create related records atomically
    ///
    /// ```rust,ignore
    /// use seaorm_django::tx;
    /// 
    /// let (author, book) = tx!(db, |txn| async move {
    ///     // Create author
    ///     let author = author::Entity::objects(txn).create(author::Model {
    ///         name: "Jane Doe".to_string(),
    ///         email: "jane@example.com".to_string(),
    ///         age: 25,
    ///         ..Default::default()
    ///     }).await?;
    ///     
    ///     // Create book - if this fails, author creation also rolls back
    ///     let book = book::Entity::objects(txn).create(book::Model {
    ///         title: "Advanced Rust".to_string(),
    ///         author_id: author.id,
    ///         price: 3999,
    ///         ..Default::default()
    ///     }).await?;
    ///     
    ///     Ok((author, book))
    /// }).await?;
    ///
    /// println!("Created author {} with book {}", author.name, book.title);
    /// ```
    ///
    /// ## Conditional rollback
    ///
    /// ```rust,ignore
    /// use seaorm_django::tx;
    /// 
    /// let result = tx!(db, |txn| async move {
    ///     let author = create_author(txn).await?;
    ///     
    ///     // Business logic validation
    ///     if author.age < 18 {
    ///         return Err(DjangoOrmError::Custom("Must be 18+".into()));
    ///     }
    ///     
    ///     update_author_count(txn).await?;
    ///     Ok(author)
    /// }).await;
    /// ```
    ///
    /// ## Bulk operations
    ///
    /// ```rust,ignore
    /// use seaorm_django::tx;
    /// 
    /// let count = tx!(db, |txn| async move {
    ///     // Delete all draft books
    ///     let deleted = book::Entity::objects(txn)
    ///         .filter(book::Column::Status.eq("draft"))
    ///         .delete()
    ///         .await?;
    ///     
    ///     // Update statistics
    ///     update_book_statistics(txn).await?;
    ///     
    ///     Ok(deleted)
    /// }).await?;
    /// ```
    ///
    /// # Thread Safety
    ///
    /// This method is safe for concurrent use. Each transaction is isolated
    /// according to the database's isolation level (typically READ COMMITTED).
    ///
    /// # Performance
    ///
    /// Transactions have overhead. Use them for operations that need atomicity,
    /// but avoid long-running transactions that lock resources.
    ///
    /// # Panics
    ///
    /// The closure should not panic. If it does, the transaction will be rolled back,
    /// but panic safety is not guaranteed for all database states.
    async fn atomic<F, T>(&self, f: F) -> Result<T, DjangoOrmError>
    where
        F: for<'a> FnOnce(&'a DatabaseTransaction) -> std::pin::Pin<Box<dyn Future<Output = Result<T, DjangoOrmError>> + Send + 'a>>,
        T: Send;

    /// Execute a closure within a savepoint (nested transaction)
    ///
    /// Savepoints allow you to create nested transactions. If the savepoint fails,
    /// only operations within it are rolled back - the outer transaction continues.
    ///
    /// # Arguments
    ///
    /// * `name` - Savepoint name (must be unique within transaction)
    /// * `f` - Async closure for savepoint operations
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use seaorm_django::tx;
    /// 
    /// tx!(db, |txn| async move {
    ///     let author = create_author(txn).await?;
    ///     
    ///     // Try to create books in a savepoint
    ///     let books_result = tx!(txn, |sp| async move {
    ///         create_book(sp, author.id, "Book 1").await?;
    ///         create_book(sp, author.id, "Book 2").await?;
    ///         Ok(2)
    ///     }).await;
    ///     
    ///     // If books fail, author is still created
    ///     let book_count = books_result.unwrap_or(0);
    ///     
    ///     Ok((author, book_count))
    /// }).await?;
    /// ```
    ///
    /// # Note
    ///
    /// Not all databases support savepoints. Check your database documentation.
    async fn savepoint<F, T>(&self, _name: &str, f: F) -> Result<T, DjangoOrmError>
    where
        F: for<'a> FnOnce(&'a DatabaseTransaction) -> std::pin::Pin<Box<dyn Future<Output = Result<T, DjangoOrmError>> + Send + 'a>>,
        T: Send;
}

// Implementation for DatabaseConnection
impl AtomicExt for DatabaseConnection {
    async fn atomic<F, T>(&self, f: F) -> Result<T, DjangoOrmError>
    where
        F: for<'a> FnOnce(&'a DatabaseTransaction) -> std::pin::Pin<Box<dyn Future<Output = Result<T, DjangoOrmError>> + Send + 'a>>,
        T: Send,
    {
        // Begin transaction
        let txn = self.begin().await?;
        
        // Execute user closure
        match f(&txn).await {
            Ok(result) => {
                // Commit on success
                txn.commit().await?;
                Ok(result)
            }
            Err(e) => {
                // Rollback on error
                txn.rollback().await?;
                Err(e)
            }
        }
    }

    async fn savepoint<F, T>(&self, _name: &str, f: F) -> Result<T, DjangoOrmError>
    where
        F: for<'a> FnOnce(&'a DatabaseTransaction) -> std::pin::Pin<Box<dyn Future<Output = Result<T, DjangoOrmError>> + Send + 'a>>,
        T: Send,
    {
        // For savepoints, we need to be within a transaction already
        // This is a simplified implementation - full savepoint support
        // would require tracking transaction depth
        self.atomic(f).await
    }
}

/// Ergonomic transaction macro (recommended!)
///
/// This macro provides Django-like transaction syntax without `Box::pin` boilerplate.
///
/// # Examples
///
/// ```rust,ignore
/// use seaorm_django::tx;
///
/// // Simple and clean - just like Django!
/// let result = tx!(db, |txn| async move {
///     let author = author::Entity::objects(txn).create(author::Model {
///         name: "John".to_string(),
///         ..Default::default()
///     }).await?;
///     Ok(author)
/// }).await?;
///
/// // Nested transactions
/// let result = tx!(db, |txn| async move {
///     let author = create_author(txn).await?;
///     
///     let books = tx!(txn, |inner| async move {
///         create_book(inner, author.id, "Book 1").await?;
///         create_book(inner, author.id, "Book 2").await?;
///         Ok(2)
///     }).await?;
///     
///     Ok((author, books))
/// }).await?;
/// ```
#[macro_export]
macro_rules! tx {
    ($db:expr, |$txn:ident| $body:expr) => {
        $db.atomic(|$txn| Box::pin($body))
    };
}

// Implementation for DatabaseTransaction (for nested transactions)
impl AtomicExt for DatabaseTransaction {
    async fn atomic<F, T>(&self, f: F) -> Result<T, DjangoOrmError>
    where
        F: for<'a> FnOnce(&'a DatabaseTransaction) -> std::pin::Pin<Box<dyn Future<Output = Result<T, DjangoOrmError>> + Send + 'a>>,
        T: Send,
    {
        // Begin nested transaction
        let txn = self.begin().await?;
        
        // Execute user closure
        match f(&txn).await {
            Ok(result) => {
                txn.commit().await?;
                Ok(result)
            }
            Err(e) => {
                txn.rollback().await?;
                Err(e)
            }
        }
    }

    async fn savepoint<F, T>(&self, _name: &str, f: F) -> Result<T, DjangoOrmError>
    where
        F: for<'a> FnOnce(&'a DatabaseTransaction) -> std::pin::Pin<Box<dyn Future<Output = Result<T, DjangoOrmError>> + Send + 'a>>,
        T: Send,
    {
        // Nested savepoint - use nested transaction
        self.atomic(f).await
    }
}

/// Alternative atomic transaction macro
///
/// **Note**: Prefer using the `tx!` macro for most cases - it's cleaner and more explicit.
///
/// This macro wraps the body in `async move {}` automatically, but `tx!` is recommended
/// because it's clearer that you're writing an async block.
///
/// # Recommended: Use `tx!` instead
///
/// ```rust,ignore
/// use seaorm_django::tx;
///
/// let author = tx!(db, |txn| async move {
///     author::Entity::objects(txn).create(author::Model {
///         name: "John".to_string(),
///         ..Default::default()
///     }).await
/// })?;
/// ```
///
/// # This macro's syntax
///
/// ```rust,ignore
/// use seaorm_django::atomic;
///
/// // Body is automatically wrapped in async move
/// let author = atomic!(db, |txn| {
///     author::Entity::objects(txn).create(author::Model {
///         name: "John".to_string(),
///         ..Default::default()
///     }).await
/// })?;
/// ```
#[macro_export]
macro_rules! atomic {
    ($db:expr, |$txn:ident| $body:expr) => {
        $db.atomic(|$txn| Box::pin(async move { $body })).await
    };
}

