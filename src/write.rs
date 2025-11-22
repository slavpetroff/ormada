//! Write operations (Create, Update, Delete) for Django-style ORM
//!
//! This module provides Django-like save/update/delete operations.
//!
//! # Examples
//!
//! ```rust,ignore
//! use seaorm_django::prelude::*;
//!
//! // Create a new record
//! let book = Book::objects(&db)
//!     .create(Book {
//!         title: "The Rust Programming Language".into(),
//!         price: 3999,
//!         published: true,
//!         ..Default::default()
//!     })
//!     .await?;
//!
//! // Save (update all fields) - Django style
//! let mut book = book;
//! book.price = 2999;
//! book.title = "Rust Book - Updated".into();
//! let updated = Book::save(&db, book).await?;
//!
//! // Bulk update with filter
//! let count = Book::objects(&db)
//!     .filter(Book::Published.eq(false))
//!     .update(|book| {
//!         book.published = true;
//!     })
//!     .await?;
//!
//! // Delete records
//! let deleted = Book::objects(&db)
//!     .filter(Book::Price.gt(10000))
//!     .delete()
//!     .await?;
//!
//! // Bulk create (high performance)
//! let books = vec![
//!     Book { title: "Book 1".into(), price: 1999, ..Default::default() },
//!     Book { title: "Book 2".into(), price: 2999, ..Default::default() },
//! ];
//! let count = Book::objects(&db)
//!     .bulk_create(books)
//!     .await?;
//! ```
//!
//! This module provides the DeleteExt trait. Create and Update operations
//! are generated directly on Entity and Model by the `#[derive(DjangoModel)]` macro.

use crate::error::DjangoOrmError;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, ModelTrait};

/// Extension trait for deleting entities
///
/// This trait is automatically implemented for all models and provides
/// Django-like delete functionality.
///
/// # Single Record Delete
///
/// ```rust,ignore
/// // Get a book and delete it
/// let book = Book::objects(db).get(1).await?;
/// book.delete(db).await?;
/// println!("Book deleted");
/// ```
///
/// # Bulk Delete
///
/// For bulk deletions, use `.delete()` on a QuerySet:
///
/// ```rust,ignore
/// // Delete all drafts
/// let count = Book::objects(db)
///     .filter(Column::Status.eq("draft"))
///     .delete()
///     .await?;
/// println!("Deleted {} drafts", count);
/// ```
///
/// # Error Handling
///
/// ```rust,ignore
/// match book.delete(db).await {
///     Ok(()) => println!("Successfully deleted"),
///     Err(DjangoOrmError::Database(e)) => {
///         // Handle database error (foreign key constraint, etc.)
///         eprintln!("Cannot delete: {}", e);
///     }
///     Err(e) => return Err(e),
/// }
/// ```
///
/// # Common Errors
///
/// - Foreign key constraint violation if other records reference this one
/// - Database connection errors
/// - Transaction errors if delete is part of a transaction
pub trait DeleteExt {
    /// Delete this entity from the database
    ///
    /// Consumes the model and removes it from the database.
    /// This operation cannot be undone.
    ///
    /// # Returns
    ///
    /// - `Ok(())` - Entity successfully deleted
    /// - `Err(DjangoOrmError::Database(_))` - Database error (constraint violation, connection, etc.)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Delete after fetching
    /// let book = Book::objects(db).get(1).await?;
    /// book.delete(db).await?;
    ///
    /// // Delete with confirmation
    /// if confirm_deletion {
    ///     book.delete(db).await?;
    /// }
    ///
    /// // Handle foreign key constraints
    /// match book.delete(db).await {
    ///     Ok(()) => println!("Deleted"),
    ///     Err(e) if e.to_string().contains("constraint") => {
    ///         println!("Cannot delete: still referenced by other records");
    ///     }
    ///     Err(e) => return Err(e),
    /// }
    /// ```
    async fn delete(self, db: &DatabaseConnection) -> Result<(), DjangoOrmError>;
}

// Blanket implementation for all model types
impl<M> DeleteExt for M
where
    M: ModelTrait + Into<<M::Entity as EntityTrait>::ActiveModel> + crate::hooks::LifecycleHooks,
    <M::Entity as EntityTrait>::ActiveModel:
        ActiveModelTrait<Entity = M::Entity> + sea_orm::ActiveModelBehavior + Send,
{
    async fn delete(self, db: &DatabaseConnection) -> Result<(), DjangoOrmError> {
        // Call before_delete hook
        self.before_delete(db).await?;

        let active_model: <M::Entity as EntityTrait>::ActiveModel = self.into();
        active_model.delete(db).await?;

        // Call after_delete hook
        // Note: We can't call hooks on the model after delete since it's consumed
        // Users should do cleanup in before_delete hook instead

        Ok(())
    }
}
