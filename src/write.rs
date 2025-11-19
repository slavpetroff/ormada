//! Write operations (Create, Update, Delete) with Django-like Model-based API
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
    async fn delete(
        self,
        db: &DatabaseConnection,
    ) -> Result<(), DjangoOrmError>;
}

// Blanket implementation for all model types
impl<M> DeleteExt for M
where
    M: ModelTrait + Into<<M::Entity as EntityTrait>::ActiveModel>,
    <M::Entity as EntityTrait>::ActiveModel: ActiveModelTrait<Entity = M::Entity>
        + sea_orm::ActiveModelBehavior
        + Send,
{
    async fn delete(
        self,
        db: &DatabaseConnection,
    ) -> Result<(), DjangoOrmError> {
        let active_model: <M::Entity as EntityTrait>::ActiveModel = self.into();
        active_model.delete(db).await?;
        Ok(())
    }
}
