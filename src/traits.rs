//! Core traits for Django-like ORM functionality

use crate::error::OrmadaError;
use sea_orm::{ConnectionTrait, EntityTrait};

/// Trait alias for connections that support Ormada-style operations
///
/// This is a marker trait for types that implement `ConnectionTrait`.
/// For transactions, use the `tx!` macro or `#[atomic]` attribute.
pub trait OrmadaConnection: ConnectionTrait {}

impl<T: ConnectionTrait> OrmadaConnection for T {}

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
        /// Column name storing the deletion timestamp (e.g., `deleted_at`)
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

/// Trait for entities that support Ormada-style creation behavior
///
/// This is automatically implemented by `#[derive(OrmadaModel)]` and `#[ormada_model]`.
/// It handles auto-increment IDs, `auto_now/auto_now_add` timestamps, and field validation.
pub trait OrmadaEntity: EntityTrait {
    /// Convert a Model to `ActiveModel` for creation with validation
    ///
    /// This method validates field constraints (`max_length`, range, etc.) before creating
    /// the `ActiveModel`. Returns an error if validation fails.
    fn to_active_model_for_create(model: Self::Model) -> Result<Self::ActiveModel, OrmadaError>;

    /// Save a model (update all fields)
    ///
    /// This ensures all fields are marked as Set so they are updated in the DB.
    /// It also handles `auto_now` fields.
    async fn save_model<C: ConnectionTrait>(
        db: &C,
        model: Self::Model,
    ) -> Result<Self::Model, OrmadaError>;

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
/// This trait is automatically implemented by the `OrmadaModel` derive macro
/// when relations are defined.
pub trait WithRelationsTrait {
    /// The base model type (same as `EntityTrait::Model`)
    type Model: Clone;

    /// The extended model type with relation accessor methods
    /// Must implement Deref to Model for accessing base fields
    type ModelWithRelations: Clone + std::ops::Deref<Target = Self::Model>;

    /// Convert a base model and typed relation data into the extended model
    ///
    /// This uses compile-time typed relations for zero-cost abstraction.
    fn from_model_and_relations<R>(model: Self::Model, relations: &R) -> Self::ModelWithRelations
    where
        Self: Sized;
}

/// Trait for types that can be viewed as a Model reference
///
/// This trait provides a unified interface for working with both `Model` and `ModelWithRelations`
/// types, allowing functions to accept either type seamlessly.
///
/// Both `Model` and `ModelWithRelations` implement this trait, enabling polymorphic behavior
/// where users only need to think about `Model` types while the ORM handles the internal
/// representation.
///
/// # Examples
///
/// ```rust,ignore
/// // Function that accepts both Model and ModelWithRelations
/// fn process_author<T: AsModelRef<Model = Author>>(author: &T) -> i32 {
///     author.as_model_ref().id
/// }
///
/// // Works with Model
/// let author: Author = Author::objects(db).create(data).await?;
/// process_author(&author);
///
/// // Works with ModelWithRelations
/// let author_with_books = Author::objects(db)
///     .prefetch_related(reverse_relations![Book])
///     .first()
///     .await?;
/// process_author(&author_with_books);
/// ```
pub trait AsModelRef {
    /// The underlying Model type
    type Model: ?Sized;

    /// Get a reference to the underlying Model
    ///
    /// For `Model`, this returns `&self`.
    /// For `ModelWithRelations`, this returns `&self.inner` via Deref.
    fn as_model_ref(&self) -> &Self::Model;
}

/// Blanket implementation for types that implement Deref
///
/// This automatically implements `AsModelRef` for both `Model` (via identity Deref)
/// and `ModelWithRelations` (via its Deref to Model implementation).
impl<T> AsModelRef for T
where
    T: std::ops::Deref,
    T::Target: Sized,
{
    type Model = T::Target;

    fn as_model_ref(&self) -> &Self::Model {
        self
    }
}

/// Extension trait for Option to enable seamless Model access
///
/// This trait provides a `.as_model()` method that converts `&Option<ModelWithRelations>`
/// to `Option<&Model>`, enabling clean function signatures.
///
/// # Examples
///
/// ```rust,ignore
/// // Clean function signature
/// fn process_author(author: Option<&Author>) {
///     if let Some(a) = author {
///         println!("{}", a.name);
///     }
/// }
///
/// // Call with Option<ModelWithRelations>
/// let author_with_relations: Option<AuthorWithRelations> = ...;
/// process_author(author_with_relations.as_model());
/// ```
pub trait OptionModelExt<M> {
    /// Convert `&Option<ModelWithRelations>` to `Option<&Model>`
    ///
    /// This enables passing `Option<ModelWithRelations>` to functions
    /// that expect `Option<&Model>`.
    fn as_model(&self) -> Option<&M>;
}

/// Implementation for `Option<T>` where `T` implements `AsRef<M>`
impl<T, M> OptionModelExt<M> for Option<T>
where
    T: AsRef<M>,
{
    fn as_model(&self) -> Option<&M> {
        self.as_ref().map(std::convert::AsRef::as_ref)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::DatabaseConnection;

    #[test]
    fn test_django_connection_trait_exists() {
        // This is a compile-time test - if it compiles, the trait works
        fn assert_django_connection<T: OrmadaConnection>() {}

        // Test that DatabaseConnection implements OrmadaConnection
        assert_django_connection::<DatabaseConnection>();
    }

    #[test]
    fn test_trait_bound_convenience() {
        // Verify that OrmadaConnection can be used as a single bound
        fn generic_with_django_connection<C: OrmadaConnection>() {}

        // This should compile with OrmadaConnection instead of multiple bounds
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
        let debug_str = format!("{config:?}");
        assert!(debug_str.contains("Enabled"));
        assert!(debug_str.contains("deleted_at"));
    }

    #[test]
    fn test_soft_delete_config_is_clone() {
        let config = SoftDeleteConfig::Enabled { column: "deleted_at" };
        let cloned = config;
        assert_eq!(config, cloned);
    }

    #[test]
    fn test_soft_delete_config_is_copy() {
        let config = SoftDeleteConfig::Enabled { column: "deleted_at" };
        let copied = config;
        let also_copied = config; // Can use again because Copy
        assert_eq!(copied, also_copied);
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
