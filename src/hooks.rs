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
//! use async_trait::async_trait;
//!
//! #[django_model(table = "books")]
//! pub struct Book {
//!     #[primary_key]
//!     pub id: i32,
//!     pub title: String,
//!     pub updated_at: DateTime<FixedOffset>,
//! }
//!
//! #[async_trait]
//! impl LifecycleHooks for Book {
//!     async fn before_save(&mut self) -> Result<(), DjangoOrmError> {
//!         self.updated_at = Utc::now().into();
//!         Ok(())
//!     }
//! }
//! ```

use crate::error::DjangoOrmError;
use async_trait::async_trait;

/// Lifecycle hooks for models - implement this trait to add custom behavior.
///
/// All methods have default no-op implementations, so you only need to override
/// the hooks you care about. Uses `#[async_trait]` for clean async signatures.
///
/// # Example
///
/// ```rust,ignore
/// #[async_trait]
/// impl LifecycleHooks for Book {
///     async fn before_save(&mut self) -> Result<(), DjangoOrmError> {
///         self.updated_at = Utc::now().into();
///         Ok(())
///     }
/// }
/// ```
#[async_trait]
pub trait LifecycleHooks: Sized + Send + Sync {
    /// Called before creating a new record (INSERT)
    async fn before_create(&mut self) -> Result<(), DjangoOrmError> {
        Ok(())
    }

    /// Called after creating a new record (INSERT)
    async fn after_create(&self) -> Result<(), DjangoOrmError> {
        Ok(())
    }

    /// Called before updating an existing record (UPDATE)
    async fn before_update(&mut self) -> Result<(), DjangoOrmError> {
        Ok(())
    }

    /// Called after updating an existing record (UPDATE)
    async fn after_update(&self) -> Result<(), DjangoOrmError> {
        Ok(())
    }

    /// Called before save (CREATE or UPDATE)
    async fn before_save(&mut self) -> Result<(), DjangoOrmError> {
        Ok(())
    }

    /// Called after save (CREATE or UPDATE)
    async fn after_save(&self) -> Result<(), DjangoOrmError> {
        Ok(())
    }

    /// Called before deleting a record (DELETE)
    async fn before_delete(&self) -> Result<(), DjangoOrmError> {
        Ok(())
    }

    /// Called after deleting a record (DELETE)
    async fn after_delete(&self) -> Result<(), DjangoOrmError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestModel {
        #[allow(dead_code)]
        value: i32,
    }

    #[async_trait]
    impl LifecycleHooks for TestModel {}

    #[tokio::test]
    async fn test_default_hooks_do_nothing() {
        let mut model = TestModel { value: 42 };

        // Test all default "before" hooks
        assert!(model.before_create().await.is_ok());
        assert!(model.before_update().await.is_ok());
        assert!(model.before_save().await.is_ok());
        assert!(model.before_delete().await.is_ok());

        // Test all default "after" hooks
        assert!(model.after_create().await.is_ok());
        assert!(model.after_update().await.is_ok());
        assert!(model.after_save().await.is_ok());
        assert!(model.after_delete().await.is_ok());
    }
}
