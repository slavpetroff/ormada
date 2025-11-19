//! Relation Registry for dynamic relation loading
//!
//! This module provides a global registry that maps entity-relation pairs to loader functions.
//! The registry enables dynamic dispatch of relation loading at runtime while maintaining
//! type safety at compile time through the macro system.

use once_cell::sync::Lazy;
use sea_orm::{DatabaseConnection, EntityTrait};
use std::any::TypeId;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

// ============================================================================
// Type Aliases
// ============================================================================

/// Map of foreign key values to boxed related models
pub type RelationMap = HashMap<i32, Box<dyn std::any::Any + Send + Sync>>;

/// Loader function signature
///
/// Takes:
/// - models: Slice of &(dyn Any + Send + Sync) (references to the entity's models)
/// - db: Static database connection
///
/// Returns:
/// - Future that resolves to a HashMap of FK -> Related Model
///
/// Uses Arc for cloneability
pub type LoaderFn = Arc<
    dyn Fn(
            &[&(dyn std::any::Any + Send + Sync)],
            &'static DatabaseConnection,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<RelationMap, crate::error::DjangoOrmError>>
                    + Send,
            >,
        > + Send
        + Sync,
>;

// ============================================================================
// Global Registry
// ============================================================================

/// Global registry of relation loaders
static RELATION_REGISTRY: Lazy<Mutex<HashMap<(TypeId, TypeId), LoaderFn>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Register a relation loader
///
/// This is typically called by macro-generated code, not directly by users.
///
/// # Parameters
///
/// - `entity_type_id`: TypeId of the entity (e.g., `TypeId::of::<book::Entity>()`)
/// - `relation_type_id`: TypeId of the related entity (e.g., `TypeId::of::<author::Entity>()`)
/// - `loader`: Function that knows how to batch-load this relation
pub fn register_loader(
    entity_type_id: TypeId,
    relation_type_id: TypeId,
    loader: LoaderFn,
) {
    RELATION_REGISTRY
        .lock()
        .unwrap()
        .insert((entity_type_id, relation_type_id), loader);
}

/// Get a registered loader
///
/// Returns None if the entity-relation pair is not registered.
pub fn get_loader(entity_type_id: TypeId, relation_type_id: TypeId) -> Option<LoaderFn> {
    RELATION_REGISTRY
        .lock()
        .unwrap()
        .get(&(entity_type_id, relation_type_id))
        .cloned()
}

// ============================================================================
// Relation Descriptor (for macro-generated registrations)
// ============================================================================

/// Descriptor for a relation
///
/// This struct is used by macro-generated code to register loaders
/// via the `inventory` crate.
pub struct RelationDescriptor {
    /// TypeId of the entity
    pub entity_type_id: TypeId,
    
    /// TypeId of the related entity
    pub relation_type_id: TypeId,
    
    /// Loader function
    pub loader: LoaderFn,
}

// Note: RelationDescriptor methods removed - no longer needed with typed relations

// ============================================================================
// Helper Traits
// ============================================================================

/// Trait for types that can extract a foreign key value
///
/// This is implemented by the macro for each entity-relation pair.
pub trait ForeignKeyExtractor<E: EntityTrait>: Send + Sync {
    /// Extract the foreign key value from a model
    fn extract_fk(model: &E::Model) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_basic() {
        let entity_type = TypeId::of::<u32>();
        let relation_type = TypeId::of::<String>();
        
        let loader: LoaderFn = std::sync::Arc::new(|_models, _db| {
            Box::pin(async move {
                Ok(HashMap::new())
            })
        });
        
        register_loader(entity_type, relation_type, loader);
        
        let retrieved = get_loader(entity_type, relation_type);
        assert!(retrieved.is_some());
    }
    
    #[test]
    fn test_registry_get_nonexistent_loader() {
        let entity_type = TypeId::of::<i64>();
        let relation_type = TypeId::of::<bool>();
        
        let retrieved = get_loader(entity_type, relation_type);
        assert!(retrieved.is_none());
    }
    
    #[test]
    fn test_registry_multiple_loaders() {
        let entity1 = TypeId::of::<i8>();
        let entity2 = TypeId::of::<i16>();
        let relation = TypeId::of::<u8>();
        
        let loader1: LoaderFn = std::sync::Arc::new(|_models, _db| {
            Box::pin(async move { Ok(HashMap::new()) })
        });
        
        let loader2: LoaderFn = std::sync::Arc::new(|_models, _db| {
            Box::pin(async move { Ok(HashMap::new()) })
        });
        
        register_loader(entity1, relation, loader1);
        register_loader(entity2, relation, loader2);
        
        assert!(get_loader(entity1, relation).is_some());
        assert!(get_loader(entity2, relation).is_some());
    }
}
