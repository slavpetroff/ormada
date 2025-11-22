//! Core traits for Django-like ORM functionality

use crate::error::DjangoOrmError;
use crate::transaction::AtomicExt;
use sea_orm::{ConnectionTrait, EntityTrait};

/// Trait alias for connections that support Django-style operations
///
/// This combines `ConnectionTrait` (SeaORM) and `AtomicExt` (Transactions).
/// Use this to avoid writing `where C: ConnectionTrait + AtomicExt`.
pub trait DjangoConnection: ConnectionTrait + AtomicExt {}

impl<T: ConnectionTrait + AtomicExt> DjangoConnection for T {}

/// Trait for entities that support Django-style creation behavior
///
/// This is automatically implemented by #[derive(DjangoModel)] and #[django_model].
/// It handles auto-increment IDs, auto_now/auto_now_add timestamps, and field validation.
pub trait DjangoEntity: EntityTrait {
    /// Convert a Model to ActiveModel for creation with validation
    ///
    /// This method validates field constraints (max_length, range, etc.) before creating
    /// the ActiveModel. Returns an error if validation fails.
    fn to_active_model_for_create(model: Self::Model) -> Result<Self::ActiveModel, DjangoOrmError>;

    /// Save a model (update all fields)
    ///
    /// This ensures all fields are marked as Set so they are updated in the DB.
    /// It also handles auto_now fields.
    async fn save_model<C: ConnectionTrait>(
        db: &C,
        model: Self::Model,
    ) -> Result<Self::Model, DjangoOrmError>;

    /// Get the soft delete field name if this entity uses soft deletes
    ///
    /// Returns the column name used for soft deletes (e.g., "deleted_at").
    /// If None, the entity uses hard deletes.
    fn soft_delete_column() -> Option<&'static str> {
        None // Default: no soft delete
    }
}

/// Trait for entities that support relation loading with the graph pattern
///
/// This trait is automatically implemented by the DjangoModel derive macro
/// when relations are defined.
pub trait WithRelationsTrait {
    /// The base model type (same as EntityTrait::Model)
    type Model: Clone;

    /// The extended model type with relation accessor methods  
    type ModelWithRelations: Clone;

    /// The type of relation data loaded
    type Relations;

    /// Convert a base model and typed relation data into the extended model
    ///
    /// This uses compile-time typed relations for zero-cost abstraction.
    fn from_model_and_relations(
        model: Self::Model,
        relations: &Self::Relations,
    ) -> Self::ModelWithRelations
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
        fn assert_django_connection<T: DjangoConnection>() {}

        // Test that DatabaseConnection implements DjangoConnection
        assert_django_connection::<DatabaseConnection>();
    }

    #[test]
    fn test_trait_bound_convenience() {
        // Verify that DjangoConnection can be used as a single bound
        fn generic_with_django_connection<C: DjangoConnection>() {}

        // This should compile with DjangoConnection instead of multiple bounds
        generic_with_django_connection::<DatabaseConnection>();
    }
}
