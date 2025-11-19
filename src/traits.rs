//! Core traits for Django-like ORM functionality

use crate::error::DjangoOrmError;
use crate::transaction::AtomicExt;
use sea_orm::{ConnectionTrait, EntityTrait};
use std::any::TypeId;
use std::collections::HashMap;

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

/// Map of loaded relations: FK -> Related Model (erased)
pub type LoadedRelationMap = HashMap<i32, Box<dyn std::any::Any + Send + Sync>>;

/// Future returning the loaded relation map
pub type LoadBatchFuture<'a> = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<LoadedRelationMap, sea_orm::DbErr>> + Send + 'a>,
>;

/// Trait for relation descriptors
///
/// Each entity-relation pair gets a descriptor that knows how to load the relation.
pub trait RelationLoader<E>: Send + Sync {
    /// The related entity type's TypeId
    fn relation_type_id(&self) -> TypeId;

    /// Load relations for a batch of models
    ///
    /// Returns a HashMap mapping foreign key values to related models.
    fn load_batch<'a>(
        &self,
        models: &[E],
        db: &'a sea_orm::DatabaseConnection,
    ) -> LoadBatchFuture<'a>;
}
