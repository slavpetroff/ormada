//! Internal re-exports for macro-generated code
//!
//! This module re-exports `SeaORM` types needed by the `#[ormada_model]` macro.
//! **Users should NOT import from this module directly.**
//!
//! All paths in generated code use `::ormada::__internal::` to ensure
//! consumers don't need `sea_orm` as a direct dependency.

#![allow(unused_imports)]

// Re-export the entire sea_orm crate for complex types that need full paths
pub use sea_orm;

// Re-export sea_query module for macro-generated code
pub use sea_orm::sea_query;

// =============================================================================
// Core Entity Types
// =============================================================================

pub use sea_orm::entity::EntityName;
pub use sea_orm::EntityTrait;
pub use sea_orm::Iden;
pub use sea_orm::IdenStatic;

// =============================================================================
// Column Types
// =============================================================================

pub use sea_orm::sea_query::ColumnDef;
pub use sea_orm::sea_query::IntoIden;
pub use sea_orm::sea_query::Nullable;
pub use sea_orm::sea_query::ValueType;
pub use sea_orm::ColumnTrait;
pub use sea_orm::ColumnType;
pub use sea_orm::ColumnTypeTrait;

// =============================================================================
// Primary Key Types
// =============================================================================

pub use sea_orm::PrimaryKeyToColumn;
pub use sea_orm::PrimaryKeyTrait;

// =============================================================================
// ActiveModel Types
// =============================================================================

pub use sea_orm::ActiveModelBehavior;
pub use sea_orm::ActiveModelTrait;
pub use sea_orm::ActiveValue;
pub use sea_orm::IntoActiveModel;
pub use sea_orm::IntoActiveValue;
pub use sea_orm::NotSet;
pub use sea_orm::Set;
pub use sea_orm::TryIntoModel;
pub use sea_orm::Unchanged;

// =============================================================================
// Relation Types
// =============================================================================

pub use sea_orm::Related;
pub use sea_orm::RelationDef;
pub use sea_orm::RelationTrait;
pub use sea_orm::RelationType;

// =============================================================================
// Query Types
// =============================================================================

pub use sea_orm::ConnectionTrait;
pub use sea_orm::FromQueryResult;
pub use sea_orm::Iterable;
pub use sea_orm::ModelTrait;
pub use sea_orm::QueryFilter;
pub use sea_orm::QueryOrder;
pub use sea_orm::QuerySelect;
pub use sea_orm::Select;
pub use sea_orm::TransactionTrait;

// =============================================================================
// Value & Expression Types
// =============================================================================

pub use sea_orm::sea_query::Expr;
pub use sea_orm::sea_query::SimpleExpr;
pub use sea_orm::sea_query::Table;
pub use sea_orm::Value;

// =============================================================================
// Database Types
// =============================================================================

pub use sea_orm::DatabaseBackend;
pub use sea_orm::DbBackend;
pub use sea_orm::DbErr;
pub use sea_orm::Schema;

// =============================================================================
// Chrono Types (for timestamps)
// =============================================================================

pub use chrono::FixedOffset;
pub use chrono::Utc;

// =============================================================================
// Serde (for serialization)
// =============================================================================

pub use ::serde::{Deserialize, Serialize};

// =============================================================================
// Async Trait
// =============================================================================

pub use async_trait::async_trait;
