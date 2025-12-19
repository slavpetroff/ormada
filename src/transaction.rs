//! Transaction support for atomic database operations (Ormada's transaction.atomic)
//!
//! This module provides Ormada-style transaction management with Rust idioms.
//! All operations within a transaction are atomic - they either all succeed or all fail.
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use ormada::prelude::*;
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
//! use ormada::prelude::*;
//!
//! #[atomic]
//! async fn create_book_with_author(
//!     db: &DatabaseConnection,
//!     title: String,
//!     author_name: String,
//! ) -> Result<Book, OrmadaError> {
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
//!         return Err(OrmadaError::Custom("Invalid price".into()));
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
//! use ormada::{prelude::*, tx};
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
//! use ormada::tx;
//!
//! let result = tx!(db, |txn| async move {
//!     let author = create_author(txn).await?;
//!
//!     // This fails - entire transaction rolls back
//!     if author.age < 18 {
//!         return Err(OrmadaError::Custom("Author must be 18+".into()));
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
//! use ormada::tx;
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

/// Ergonomic transaction macro with static dispatch
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
/// use ormada::tx;
///
/// // Simple and clean - just like Ormada!
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
            use $crate::__internal::TransactionTrait;

            // Begin transaction - static dispatch, no boxing
            let __txn = $db.begin().await.map_err($crate::error::OrmadaError::from)?;

            // Execute the body directly - user controls the return type
            let __result: Result<_, $crate::error::OrmadaError> = (|| async {
                let $txn = &__txn;
                $body
            })()
            .await;

            // Commit or rollback based on result
            match __result {
                Ok(__value) => {
                    __txn.commit().await.map_err($crate::error::OrmadaError::from)?;
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
