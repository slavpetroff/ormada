//! Transaction support for atomic database operations (Django's transaction.atomic)
//!
//! This module provides Django-style transaction management with Rust idioms.
//! All operations within a transaction are atomic - they either all succeed or all fail.
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use seaorm_django::prelude::*;
//!
//! // Simple transaction with tx! macro
//! let (author, book) = tx!(db, |txn| async move {
//!     // Create author
//!     let author = Author::objects(txn)
//!         .create(Author {
//!             name: "John Doe".into(),
//!             ..Default::default()
//!         })
//!         .await?;
//!     
//!     // Create book - if this fails, author creation also rolls back
//!     let book = Book::objects(txn)
//!         .create(Book {
//!             title: "My Book".into(),
//!             author_id: author.id,
//!             ..Default::default()
//!         })
//!         .await?;
//!     
//!     Ok((author, book))
//! }).await?;
//! ```
//!
//! # Using #[atomic] Attribute
//!
//! ```rust,ignore
//! use seaorm_django::prelude::*;
//!
//! #[atomic]
//! async fn create_book_with_author(
//!     db: &DatabaseConnection,
//!     title: String,
//!     author_name: String,
//! ) -> Result<Book, DjangoOrmError> {
//!     // This entire function runs in a transaction
//!     let author = Author::objects(db)
//!         .create(Author {
//!             name: author_name,
//!             ..Default::default()
//!         })
//!         .await?;
//!     
//!     Book::objects(db)
//!         .create(Book {
//!             title,
//!             author_id: author.id,
//!             ..Default::default()
//!         })
//!         .await
//! }
//! ```
//!
//! # Error Handling
//!
//! ```rust,ignore
//! // Transactions automatically rollback on error
//! let result = tx!(db, |txn| async move {
//!     let book = Book::objects(txn)
//!         .create(Book { /* ... */ })
//!         .await?;
//!     
//!     // If this fails, book creation is rolled back
//!     if book.price < 0 {
//!         return Err(DjangoOrmError::Custom("Invalid price".into()));
//!     }
//!     
//!     Ok(book)
//! }).await;
//!
//! match result {
//!     Ok(book) => println!("Book created: {}", book.title),
//!     Err(e) => println!("Transaction failed: {}", e),
//! }
//! ```
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
//!     let author = Author::objects(txn).create(Author {
//!         name: "John Doe".to_string(),
//!         email: "john@example.com".to_string(),
//!         age: 30,
//!         ..Default::default()
//!     }).await?;
//!     
//!     // Create book referencing the author
//!     let book = Book::objects(txn).create(Book {
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
    ///     let author = Author::objects(txn).create(Author {
    ///         name: "Jane Doe".to_string(),
    ///         email: "jane@example.com".to_string(),
    ///         age: 25,
    ///         ..Default::default()
    ///     }).await?;
    ///     
    ///     // Create book - if this fails, author creation also rolls back
    ///     let book = Book::objects(txn).create(Book {
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
    ///     let deleted = Book::objects(txn)
    ///         .filter(Book::Status.eq("draft"))
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
        F: for<'a> FnOnce(
            &'a DatabaseTransaction,
        ) -> std::pin::Pin<
            Box<dyn Future<Output = Result<T, DjangoOrmError>> + Send + 'a>,
        >,
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
        F: for<'a> FnOnce(
            &'a DatabaseTransaction,
        ) -> std::pin::Pin<
            Box<dyn Future<Output = Result<T, DjangoOrmError>> + Send + 'a>,
        >,
        T: Send;
}

// Implementation for DatabaseConnection
impl AtomicExt for DatabaseConnection {
    async fn atomic<F, T>(&self, f: F) -> Result<T, DjangoOrmError>
    where
        F: for<'a> FnOnce(
            &'a DatabaseTransaction,
        ) -> std::pin::Pin<
            Box<dyn Future<Output = Result<T, DjangoOrmError>> + Send + 'a>,
        >,
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
        F: for<'a> FnOnce(
            &'a DatabaseTransaction,
        ) -> std::pin::Pin<
            Box<dyn Future<Output = Result<T, DjangoOrmError>> + Send + 'a>,
        >,
        T: Send,
    {
        // For savepoints, we need to be within a transaction already
        // This is a simplified implementation - full savepoint support
        // would require tracking transaction depth
        self.atomic(f).await
    }
}

/// Ergonomic transaction macro with static dispatch (recommended!)
///
/// This macro provides Django-like transaction syntax with zero boxing overhead.
/// Uses **static dispatch** - no `Box::pin` or `dyn Future`.
///
/// # Syntax
///
/// ```rust,ignore
/// tx!(connection, |txn| async move {
///     // Your transaction code here
///     Ok(result)
/// })
/// ```
///
/// **Important:** The body MUST be `async move { ... }` with braces.
///
/// # Examples
///
/// ```rust,ignore
/// use seaorm_django::tx;
///
/// // Simple and clean - just like Django!
/// let result = tx!(db, |txn| async move {
///     let author = Author::objects(txn).create(Author {
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
///
/// # Static Dispatch Benefits
///
/// - **Zero heap allocation** - no `Box::pin`
/// - **No vtable lookup** - concrete future types
/// - **Better optimization** - compiler can inline
#[macro_export]
macro_rules! tx {
    ($db:expr, |$txn:ident| async move $body:block) => {
        async {
            use sea_orm::TransactionTrait;

            // Begin transaction - static dispatch, no boxing
            let __txn = $db.begin().await.map_err($crate::error::DjangoOrmError::from)?;

            // Execute the body directly - user controls the return type
            let __result: Result<_, $crate::error::DjangoOrmError> = (|| async {
                let $txn = &__txn;
                $body
            })()
            .await;

            // Commit or rollback based on result
            match __result {
                Ok(__value) => {
                    __txn.commit().await.map_err($crate::error::DjangoOrmError::from)?;
                    Ok(__value)
                }
                Err(__err) => {
                    let _ = __txn.rollback().await;
                    Err(__err)
                }
            }
        }
    };
}

// Implementation for DatabaseTransaction (for nested transactions)
impl AtomicExt for DatabaseTransaction {
    async fn atomic<F, T>(&self, f: F) -> Result<T, DjangoOrmError>
    where
        F: for<'a> FnOnce(
            &'a Self,
        ) -> std::pin::Pin<
            Box<dyn Future<Output = Result<T, DjangoOrmError>> + Send + 'a>,
        >,
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
        F: for<'a> FnOnce(
            &'a Self,
        ) -> std::pin::Pin<
            Box<dyn Future<Output = Result<T, DjangoOrmError>> + Send + 'a>,
        >,
        T: Send,
    {
        // Nested savepoint - use nested transaction
        self.atomic(f).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_ext_trait_on_database_connection() {
        // Compile-time test: DatabaseConnection implements AtomicExt
        fn assert_implements_atomic_ext<T: AtomicExt>() {}
        assert_implements_atomic_ext::<DatabaseConnection>();
    }

    #[test]
    fn test_atomic_ext_trait_on_database_transaction() {
        // Compile-time test: DatabaseTransaction implements AtomicExt
        fn assert_implements_atomic_ext<T: AtomicExt>() {}
        assert_implements_atomic_ext::<DatabaseTransaction>();
    }
}
