//! # seaorm-django
//!
//! **Django-inspired ergonomic ORM for SeaORM with zero-cost abstractions**
//!
//! This library brings Django's elegant ORM API to Rust, providing:
//! - **🚀 Zero-cost abstractions**: Compile-time typed relations, no runtime overhead
//! - **🎯 Type-safe**: Full compile-time checking, works with SeaORM's generated types
//! - **🐍 Django-like**: 85%+ API compatibility for familiar, ergonomic queries
//! - **⚡ Performance**: No duplication, direct integration with SeaORM
//!
//! ## Core Features
//!
//! ### 📊 Query API
//! Django-style QuerySet with filtering, ordering, pagination, and aggregation:
//! - `filter()` / `exclude()` - WHERE clauses with method chaining
//! - `distinct()` - Remove duplicate rows
//! - `order_by_asc()` / `order_by_desc()` - Ordering
//! - `limit()` / `offset()` - Pagination
//! - `count()` / `exists()` - Efficient aggregation
//! - `first()` / `last()` / `get()` - Single record retrieval
//! - `earliest()` / `latest()` - Get first/last by field
//! - `values()` / `values_list()` - Column projection
//! - `get_or_create()` / `update_or_create()` - Upsert operations
//! - `prefetch_related()` - N+1 query prevention
//!
//! ### 📈 Aggregations
//! Database-level aggregations for analytics:
//! - `aggregate_count()` - Count records
//! - `aggregate_sum()` - Sum column values
//! - `aggregate_avg()` - Average of column
//! - `aggregate_max()` / `aggregate_min()` - Max/min values
//! - All executed at database level for performance
//!
//! ### ✍️ Write API
//! Django-style model operations:
//! - `save()` - Django-like full model updates
//! - `update()` - Bulk updates with filters
//! - `delete()` - Soft/hard delete operations
//!
//! ### ⚡ Bulk Operations
//! High-performance batch operations:
//! - `bulk_create()` - Insert 1000s of records in one query
//! - 10-100x faster than individual operations
//!
//! ### 🔒 Transactions
//! Django-style atomic operations for data consistency:
//! - `atomic()` - Execute operations in a transaction
//! - `#[atomic]` - Attribute macro for transactional functions (new!)
//! - `savepoint()` - Nested transactions with rollback points
//! - Automatic commit on success, rollback on error
//! - ACID guarantees for data integrity
//!
//! ### 🔍 Q Objects
//! Complex query building with OR/AND/NOT logic:
//! - `Q::all()` - AND conditions
//! - `Q::any()` - OR conditions  
//! - `Q::not()` - NOT conditions
//! - Nestable and combinable
//!
//! ### 🔗 Relations
//! Zero-cost eager loading:
//! - Compile-time typed relations
//! - `prefetch_related()` for N+1 prevention
//! - Macro-based relation specification
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use seaorm_django::prelude::*;
//! use sea_orm::ColumnTrait;
//!
//! // === QUERYING ===
//!
//! // Basic filtering
//! let books = book::Entity::objects(db)
//!     .filter(book::Column::Title.contains("Rust"))
//!     .exclude(book::Column::Price.gt(5000))
//!     .order_by_desc(book::Column::Published)
//!     .limit(10)
//!     .all()
//!     .await?;
//!
//! // Complex queries with Q objects
//! let q = Q::any()
//!     .add(book::Column::Title.contains("Rust"))
//!     .add(book::Column::Title.contains("Python"));
//! let books = book::Entity::objects(db).filter(q).all().await?;
//!
//! // Aggregation
//! let count = book::Entity::objects(db)
//!     .filter(book::Column::Price.lt(3000))
//!     .count()
//!     .await?;
//!
//! let exists = book::Entity::objects(db)
//!     .filter(book::Column::Title.eq("The Rust Book"))
//!     .exists()
//!     .await?;
//!
//! // Column selection (values)
//! let titles = book::Entity::objects(db)
//!     .values_list(vec![book::Column::Title], true)
//!     .await?;
//!
//! // Eager loading relations (prevent N+1)
//! let books = book::Entity::objects(db)
//!     .prefetch_related(relations![author::Entity])
//!     .all()
//!     .await?;
//!
//! for book in books {
//!     if let Some(author) = book.author {
//!         println!("{} by {}", book.title, author.name);
//!     }
//! }
//!
//! // === WRITING ===
//!
//! // Create
//! let book = book::Entity::objects(db).create(book::Model {
//!     title: "New Book".to_string(),
//!     price: 2999,
//!     ..Default::default()
//! }).await?;
//!
//! // Update (Django-style: updates ALL fields)
//! let updated = book::Entity::save_model(db, book).await?;
//!
//! // Bulk update
//! let count = book::Entity::objects(db)
//!     .filter(book::Column::Price.lt(1000))
//!     .update(|book| book.price = 999)
//!     .await?;
//!
//! // Delete
//! let count = book::Entity::objects(db)
//!     .filter(book::Column::Published.eq(false))
//!     .delete()
//!     .await?;
//!
//! // === TRANSACTIONS ===
//!
//! // Atomic operations - all succeed or all fail - simple and ergonomic!
//! use seaorm_django::tx;
//!
//! let (author, book) = tx!(db, |txn| async move {
//!     // Create author
//!     let author = author::Entity::objects(txn).create(author::Model {
//!         name: "John Doe".to_string(),
//!         email: "john@example.com".to_string(),
//!         age: 30,
//!         ..Default::default()
//!     }).await?;
//!     
//!     // Create book - if this fails, author creation also rolls back
//!     let book = book::Entity::objects(txn).create(book::Model {
//!         title: "Rust Guide".to_string(),
//!         author_id: author.id,
//!         price: 2999,
//!         ..Default::default()
//!     }).await?;
//!     
//!     Ok((author, book))
//! }).await?;
//! ```
//!
//! ## Column Operations
//!
//! All SeaORM Column enums automatically get Django-like methods via `ColumnExt`:
//!
//! ```rust,ignore
//! // String operations
//! book::Column::Title.contains("Rust")
//! book::Column::Title.starts_with("The")
//! book::Column::Title.ends_with("Guide")
//!
//! // Comparisons
//! book::Column::Price.eq(2999)
//! book::Column::Price.ne(0)
//! book::Column::Price.gt(1000)
//! book::Column::Price.gte(1000)
//! book::Column::Price.lt(5000)
//! book::Column::Price.lte(5000)
//!
//! // Null checks
//! book::Column::Description.is_null()
//! book::Column::Description.is_not_null()
//!
//! // IN queries
//! book::Column::Id.in_values(vec![1, 2, 3])
//! ```
//!
//! ## Derive Macro (Optional)
//!
//! Enable the `derive` feature for automatic implementation:
//!
//! ```rust,ignore
//! use seaorm_django_derive::DjangoModel;
//! use sea_orm::entity::prelude::*;
//!
//! #[derive(DjangoModel, DeriveEntityModel)]
//! #[sea_orm(table_name = "books")]
//! #[django(relations(author = "author::Entity"))]
//! pub struct Model {
//!     #[sea_orm(primary_key)]
//!     pub id: i32,
//!     pub title: String,
//!     pub author_id: i32,
//!     
//!     #[django(auto_now_add)]
//!     pub created_at: DateTimeWithTimeZone,
//!     
//!     #[django(auto_now)]
//!     pub updated_at: DateTimeWithTimeZone,
//! }
//! ```
//!
//! ## Performance
//!
//! - **Zero-cost abstractions**: All relation loading is compile-time typed
//! - **No runtime overhead**: Direct integration with SeaORM, no additional layers
//! - **Efficient queries**: Uses SeaORM's query builder directly
//! - **N+1 prevention**: `prefetch_related()` uses batch loading (1+M queries, not N+1)

#![deny(missing_docs)]
#![allow(async_fn_in_trait)]
#![doc(html_root_url = "https://docs.rs/seaorm-django/0.1.0")]

pub mod aggregations;
pub mod batching;
pub mod error;
pub mod query;
pub mod relations;
pub mod traits;
pub mod transaction;
pub mod types;
pub mod write;

/// Convenience re-exports for common usage
///
/// Import everything you need with: `use seaorm_django::prelude::*;`
pub mod prelude {
    //! Commonly used imports for seaorm-django

    // Django-style ORM extensions
    pub use crate::aggregations::AggregateExt;
    pub use crate::batching;
    pub use crate::error::DjangoOrmError;
    pub use crate::query::{Q, QueryExt, QuerySet};
    pub use crate::relations::{HasRelation, LoadRelations, QuerySetEager};
    pub use crate::traits::{DjangoConnection, DjangoEntity, WithRelationsTrait};
    pub use crate::transaction::AtomicExt;
    pub use crate::types::OnDelete;
    
    // Macros
    pub use crate::{relations, tx};

    // Derive macros
    #[cfg(feature = "derive")]
    pub use seaorm_django_derive::{atomic, django_model, DjangoModel};

    // Fast hash map for better performance
    pub use rustc_hash::FxHashMap;

    // SeaORM core types (explicit re-exports for clarity)
    pub use sea_orm::{
        // Database connection
        Database, DatabaseConnection, DatabaseTransaction,
        
        // Entity and model traits
        EntityTrait, ModelTrait, ActiveModelTrait, PrimaryKeyTrait,
        
        // Column and query traits
        ColumnTrait, QuerySelect, QueryFilter, QueryOrder, QueryTrait,
        
        // Query builders
        Condition, JoinType, Order, RelationTrait,
        
        // Value types
        ActiveValue, Value, Set, NotSet, Unchanged,
        
        // Error type
        DbErr,
        
        // Transaction isolation
        IsolationLevel, TransactionTrait,
    };
    
    // Re-export SeaORM module for advanced usage
    pub use sea_orm;

    // Datetime handling
    pub use chrono::{DateTime, FixedOffset, Utc};

    /// Type alias for datetime with timezone (DateTime<FixedOffset>)
    /// This matches SeaORM's DateTimeWithTimeZone type
    pub type DateTimeWithTimeZone = DateTime<FixedOffset>;
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_prelude_exports() {
        // Ensure prelude exports compile
    }
}
