//! Lifecycle hooks for models
//!
//! This module provides lifecycle hooks similar to Django's signals and model hooks.
//! Hooks are methods that are automatically called at specific points in a model's lifecycle.
//!
//! # Available Hooks
//!
//! - `before_create` - Called before INSERT
//! - `after_create` - Called after INSERT
//! - `before_update` - Called before UPDATE
//! - `after_update` - Called after UPDATE
//! - `before_save` - Called before CREATE or UPDATE
//! - `after_save` - Called after CREATE or UPDATE
//! - `before_delete` - Called before DELETE
//! - `after_delete` - Called after DELETE
//!
//! # Example
//!
//! ```rust,ignore
//! use seaorm_django::prelude::*;
//!
//! #[django_model(table = "books")]
//! pub struct Book {
//!     #[primary_key]
//!     pub id: i32,
//!     pub title: String,
//!     pub updated_at: DateTime<FixedOffset>,
//! }
//!
//! // Implement hooks on your model type
//! impl AsyncLifecycleHooks for Book {
//!     async fn before_save(&mut self) -> Result<(), DjangoOrmError> {
//!         self.updated_at = Utc::now().into();
//!         Ok(())
//!     }
//!     
//!     async fn after_create(&self, _db: &impl ConnectionTrait) -> Result<(), DjangoOrmError> {
//!         println!("Book created: {}", self.title);
//!         Ok(())
//!     }
//! }
//! ```

use crate::error::DjangoOrmError;
use sea_orm::ConnectionTrait;
use std::future::Future;
use std::pin::Pin;

/// Type alias for async hook functions that can modify the model
pub type BeforeHookFn<M> = for<'a> fn(&'a mut M) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + 'a>>;

/// User-friendly alias for implementing lifecycle hooks.
///
/// Users implement this trait on their model with simple methods that return futures.
/// The return type is `Pin<Box<...>>` but users write async blocks which are auto-boxed.
///
/// # Example
///
/// ```rust,ignore
/// impl AsyncLifecycleHooks for Book {
///     fn before_save(&mut self) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
///         Box::pin(async {
///             self.updated_at = Utc::now().into();
///             Ok(())
///         })
///     }
/// }
/// ```
pub trait AsyncLifecycleHooks: Sized + Send {
    /// Called before creating a new record (INSERT)
    fn before_create(&mut self) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    /// Called after creating a new record (INSERT)
    fn after_create<C: ConnectionTrait>(&self, _db: &C) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    /// Called before updating an existing record (UPDATE)
    fn before_update(&mut self) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    /// Called after updating an existing record (UPDATE)
    fn after_update<C: ConnectionTrait>(&self, _db: &C) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    /// Called before save (CREATE or UPDATE)
    fn before_save(&mut self) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    /// Called after save (CREATE or UPDATE)
    fn after_save<C: ConnectionTrait>(&self, _db: &C) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    /// Called before deleting a record (DELETE)
    fn before_delete<C: ConnectionTrait>(&self, _db: &C) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    /// Called after deleting a record (DELETE)
    fn after_delete<C: ConnectionTrait>(&self, _db: &C) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }
}


/// Internal trait for lifecycle hooks - users should implement `AsyncLifecycleHooks` instead.
///
/// This trait is what the ORM internally uses. It's automatically provided for
/// any type that implements `AsyncLifecycleHooks`.
pub trait LifecycleHooks: Sized {
    /// Called before creating a new record (INSERT)
    ///
    /// Use this to modify the model before it's inserted.
    /// Hook can be sync or async.
    fn before_create(&mut self) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    /// Called after creating a new record (INSERT)
    ///
    /// Use this for post-creation side effects like sending emails,
    /// logging, cache updates, etc.
    fn after_create<C: ConnectionTrait>(&self, _db: &C) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    /// Called before updating an existing record (UPDATE)
    ///
    /// Use this to modify the model before it's updated.
    fn before_update(&mut self) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    /// Called after updating an existing record (UPDATE)
    ///
    /// Use this for post-update side effects.
    fn after_update<C: ConnectionTrait>(&self, _db: &C) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    /// Called before save (CREATE or UPDATE)
    ///
    /// This is called for both create and update operations.
    /// Use this for common logic that should run on all saves.
    fn before_save(&mut self) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    /// Called after save (CREATE or UPDATE)
    ///
    /// This is called for both create and update operations.
    fn after_save<C: ConnectionTrait>(&self, _db: &C) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    /// Called before deleting a record (DELETE)
    ///
    /// Use this for cleanup operations, cascade deletes, etc.
    /// Note: This is only called for single-record deletes, not bulk deletes.
    fn before_delete<C: ConnectionTrait>(&self, _db: &C) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    /// Called after deleting a record (DELETE)
    ///
    /// Use this for post-deletion side effects like cache invalidation.
    /// Note: This is only called for single-record deletes, not bulk deletes.
    fn after_delete<C: ConnectionTrait>(&self, _db: &C) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }
}

/// Blanket implementation: AsyncLifecycleHooks IS LifecycleHooks
/// This forwards all the AsyncLifecycleHooks methods to LifecycleHooks
impl<T> LifecycleHooks for T 
where 
    T: AsyncLifecycleHooks 
{
    fn before_create(&mut self) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        <Self as AsyncLifecycleHooks>::before_create(self)
    }

    fn after_create<C: ConnectionTrait>(&self, db: &C) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        <Self as AsyncLifecycleHooks>::after_create(self, db)
    }

    fn before_update(&mut self) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        <Self as AsyncLifecycleHooks>::before_update(self)
    }

    fn after_update<C: ConnectionTrait>(&self, db: &C) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        <Self as AsyncLifecycleHooks>::after_update(self, db)
    }

    fn before_save(&mut self) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        <Self as AsyncLifecycleHooks>::before_save(self)
    }

    fn after_save<C: ConnectionTrait>(&self, db: &C) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        <Self as AsyncLifecycleHooks>::after_save(self, db)
    }

    fn before_delete<C: ConnectionTrait>(&self, db: &C) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        <Self as AsyncLifecycleHooks>::before_delete(self, db)
    }

    fn after_delete<C: ConnectionTrait>(&self, db: &C) -> Pin<Box<dyn Future<Output = Result<(), DjangoOrmError>> + Send + '_>> {
        <Self as AsyncLifecycleHooks>::after_delete(self, db)
    }
}

/// Helper to call multiple hooks in sequence
///
/// Stops at first error and returns it.
pub async fn call_hooks<M, F, Fut>(model: &mut M, hooks: &[F]) -> Result<(), DjangoOrmError>
where
    F: Fn(&mut M) -> Fut,
    Fut: Future<Output = Result<(), DjangoOrmError>>,
{
    for hook in hooks {
        hook(model).await?;
    }
    Ok(())
}

/// Helper to call read-only hooks in sequence with database connection
pub async fn call_readonly_hooks<M, C, F, Fut>(
    model: &M,
    db: &C,
    hooks: &[F],
) -> Result<(), DjangoOrmError>
where
    C: ConnectionTrait,
    F: Fn(&M, &C) -> Fut,
    Fut: Future<Output = Result<(), DjangoOrmError>>,
{
    for hook in hooks {
        hook(model, db).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    struct TestModel {
        value: i32,
    }
    
    impl LifecycleHooks for TestModel {}
    
    #[tokio::test]
    async fn test_default_hooks_do_nothing() {
        let mut model = TestModel { value: 42 };
        
        // All default hooks should succeed and do nothing
        assert!(model.before_create().await.is_ok());
        assert!(model.before_update().await.is_ok());
        assert!(model.before_save().await.is_ok());
    }
}
