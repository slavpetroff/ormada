//! Core traits for Django-like ORM functionality

use crate::error::ErgormError;
use sea_orm::{ConnectionTrait, EntityTrait};

/// Trait alias for connections that support Django-style operations
///
/// This is a marker trait for types that implement `ConnectionTrait`.
/// For transactions, use the `tx!` macro or `#[atomic]` attribute.
pub trait ErgormConnection: ConnectionTrait {}

impl<T: ConnectionTrait> ErgormConnection for T {}

// ============================================================================
// Entity Capability Enums
// ============================================================================

/// Soft delete configuration for an entity
///
/// This enum clearly expresses whether an entity supports soft deletion
/// and which column is used to track it.
///
/// # Examples
///
/// ```rust,ignore
/// // Entity with soft delete
/// fn soft_delete() -> SoftDeleteConfig {
///     SoftDeleteConfig::Enabled { column: "deleted_at" }
/// }
///
/// // Entity without soft delete (default)
/// fn soft_delete() -> SoftDeleteConfig {
///     SoftDeleteConfig::Disabled
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoftDeleteConfig {
    /// Soft delete is disabled - records are permanently deleted
    Disabled,
    /// Soft delete is enabled - records are marked with a timestamp
    Enabled {
        /// Column name storing the deletion timestamp (e.g., "deleted_at")
        column: &'static str,
    },
}

impl SoftDeleteConfig {
    /// Check if soft delete is enabled
    pub const fn is_enabled(&self) -> bool {
        matches!(self, Self::Enabled { .. })
    }

    /// Get the column name if soft delete is enabled
    pub const fn column(&self) -> Option<&'static str> {
        match self {
            Self::Disabled => None,
            Self::Enabled { column } => Some(column),
        }
    }
}

impl Default for SoftDeleteConfig {
    fn default() -> Self {
        Self::Disabled
    }
}

/// Trait for entities that support Django-style creation behavior
///
/// This is automatically implemented by #[derive(DjangoModel)] and #[`django_model`].
/// It handles auto-increment IDs, `auto_now/auto_now_add` timestamps, and field validation.
pub trait ErgormEntity: EntityTrait {
    /// Convert a Model to `ActiveModel` for creation with validation
    ///
    /// This method validates field constraints (`max_length`, range, etc.) before creating
    /// the `ActiveModel`. Returns an error if validation fails.
    fn to_active_model_for_create(model: Self::Model) -> Result<Self::ActiveModel, ErgormError>;

    /// Save a model (update all fields)
    ///
    /// This ensures all fields are marked as Set so they are updated in the DB.
    /// It also handles `auto_now` fields.
    async fn save_model<C: ConnectionTrait>(
        db: &C,
        model: Self::Model,
    ) -> Result<Self::Model, ErgormError>;

    /// Get soft delete configuration for this entity
    ///
    /// Returns `SoftDeleteConfig::Enabled { column }` if the entity uses soft deletes,
    /// or `SoftDeleteConfig::Disabled` (default) for hard deletes.
    fn soft_delete() -> SoftDeleteConfig {
        SoftDeleteConfig::Disabled
    }
}

/// Trait for entities that support relation loading with the graph pattern
///
/// This trait is automatically implemented by the `DjangoModel` derive macro
/// when relations are defined.
pub trait WithRelationsTrait {
    /// The base model type (same as `EntityTrait::Model`)
    type Model: Clone;

    /// The extended model type with relation accessor methods  
    type ModelWithRelations: Clone;

    /// Convert a base model and typed relation data into the extended model
    ///
    /// This uses compile-time typed relations for zero-cost abstraction.
    fn from_model_and_relations<R>(model: Self::Model, relations: &R) -> Self::ModelWithRelations
    where
        Self: Sized;
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::DatabaseConnection;

    #[test]
    fn test_django_connection_trait_exists() {
        // This is a compile-time test - if it compiles, the trait works
        fn assert_django_connection<T: ErgormConnection>() {}

        // Test that DatabaseConnection implements ErgormConnection
        assert_django_connection::<DatabaseConnection>();
    }

    #[test]
    fn test_trait_bound_convenience() {
        // Verify that ErgormConnection can be used as a single bound
        fn generic_with_django_connection<C: ErgormConnection>() {}

        // This should compile with ErgormConnection instead of multiple bounds
        generic_with_django_connection::<DatabaseConnection>();
    }

    // ========================================================================
    // SoftDeleteConfig Enum Tests
    // ========================================================================

    #[test]
    fn test_soft_delete_config_disabled() {
        let config = SoftDeleteConfig::Disabled;
        assert!(!config.is_enabled());
        assert_eq!(config.column(), None);
    }

    #[test]
    fn test_soft_delete_config_enabled() {
        let config = SoftDeleteConfig::Enabled { column: "deleted_at" };
        assert!(config.is_enabled());
        assert_eq!(config.column(), Some("deleted_at"));
    }

    #[test]
    fn test_soft_delete_config_default() {
        let config = SoftDeleteConfig::default();
        assert_eq!(config, SoftDeleteConfig::Disabled);
    }

    #[test]
    fn test_soft_delete_config_is_debug() {
        let config = SoftDeleteConfig::Enabled { column: "deleted_at" };
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("Enabled"));
        assert!(debug_str.contains("deleted_at"));
    }

    #[test]
    fn test_soft_delete_config_is_clone() {
        let config = SoftDeleteConfig::Enabled { column: "deleted_at" };
        let cloned = config.clone();
        assert_eq!(config, cloned);
    }

    #[test]
    fn test_soft_delete_config_is_copy() {
        let config = SoftDeleteConfig::Enabled { column: "deleted_at" };
        let copied = config;
        let _also_copied = config; // Can use again because Copy
        assert_eq!(copied, config);
    }

    #[test]
    fn test_soft_delete_config_pattern_matching() {
        let configs =
            vec![SoftDeleteConfig::Disabled, SoftDeleteConfig::Enabled { column: "deleted_at" }];

        for config in configs {
            match config {
                SoftDeleteConfig::Disabled => assert!(!config.is_enabled()),
                SoftDeleteConfig::Enabled { column } => {
                    assert!(config.is_enabled());
                    assert_eq!(column, "deleted_at");
                }
            }
        }
    }
}
