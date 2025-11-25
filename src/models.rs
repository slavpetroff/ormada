//! Model and entity types
//!
//! This module provides model-related types, mirroring Django's `django.db.models`.
//! Users should import from here instead of using `sea_orm` directly.
//!
//! # Usage
//!
//! ```rust,ignore
//! use seaorm_django::models::{EntityTrait, ModelTrait};
//! ```

// Entity trait - core trait for all entities
pub use sea_orm::EntityTrait;

// Model trait - for model instances
pub use sea_orm::ModelTrait;

// Active model traits - for mutations
pub use sea_orm::ActiveModelBehavior;
pub use sea_orm::ActiveModelTrait;
pub use sea_orm::IntoActiveModel;

// Related entity types
pub use sea_orm::Related;
pub use sea_orm::RelationDef;
pub use sea_orm::RelationTrait;
pub use sea_orm::RelationType;

// Query traits
pub use sea_orm::FromQueryResult;
pub use sea_orm::QueryFilter;
pub use sea_orm::QueryOrder;
pub use sea_orm::QuerySelect;
pub use sea_orm::QueryTrait;

// JSON values for .values() queries
pub use sea_orm::JsonValue;

// Select types
pub use sea_orm::Select;
pub use sea_orm::SelectTwo;
pub use sea_orm::SelectTwoMany;

// Insert/Update types
pub use sea_orm::Insert;
pub use sea_orm::InsertResult;
pub use sea_orm::Update;
pub use sea_orm::UpdateResult;

// Delete types
pub use sea_orm::Delete;
pub use sea_orm::DeleteMany;
pub use sea_orm::DeleteResult;

// Paginator
pub use sea_orm::Paginator;
pub use sea_orm::PaginatorTrait;
