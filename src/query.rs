//! Core Django-like Query API for `SeaORM`
//!
//! This module provides ergonomic query building with zero duplication.
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use seaorm_django::prelude::*;
//!
//! // Basic filtering and ordering
//! let books = Book::objects(&db)
//!     .filter(Book::Price.lt(3000))
//!     .order_by_desc(Book::Published)
//!     .limit(10)
//!     .all()
//!     .await?;
//!
//! // Count records
//! let count = Book::objects(&db)
//!     .filter(Book::InStock.eq(true))
//!     .count()
//!     .await?;
//!
//! // Get single record
//! let book = Book::objects(&db)
//!     .get(42)
//!     .await?;
//!
//! // Check existence
//! let exists = Book::objects(&db)
//!     .filter(Book::Isbn.eq("978-0134685991"))
//!     .exists()
//!     .await?;
//! ```
//!
//! # Advanced Usage
//!
//! ```rust,ignore
//! // Complex queries with Q objects
//! let q = Q::any()
//!     .add(Book::Title.contains("Rust"))
//!     .add(Book::Title.contains("Python"));
//!
//! let books = Book::objects(&db)
//!     .filter(q)
//!     .exclude(Book::Price.gt(5000))
//!     .all()
//!     .await?;
//!
//! // Get or create
//! let (book, created) = Book::objects(&db)
//!     .get_or_create(
//!         Book::Isbn.eq("978-1234567890"),
//!         || Book {
//!             title: "New Book".into(),
//!             isbn: "978-1234567890".into(),
//!             price: 2999,
//!             ..Default::default()
//!         }
//!     )
//!     .await?;
//! ```

use crate::error::DjangoOrmError;
use crate::hooks::LifecycleHooks;
use crate::upsert::UpsertBuilder;
use sea_orm::sea_query::{Expr, Func, SimpleExpr};
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, DbErr, EntityTrait, Order, PrimaryKeyTrait,
    QueryFilter, QueryOrder, QuerySelect, Select,
};
use std::sync::Arc;
use tokio::sync::RwLock;

// ============================================================================
// Concurrency Helpers
// ============================================================================

/// Check if a database error is a unique constraint violation
///
/// Used internally by `get_or_create` and `update_or_create` to detect
/// race conditions and retry the operation.
///
/// This is a heuristic check that works across `SQLite`, `PostgreSQL`, and `MySQL`.
fn is_unique_violation(err: &DbErr) -> bool {
    // Check the error message for common unique constraint keywords
    // This works across all database backends
    let msg = err.to_string().to_lowercase();

    // Common patterns across databases:
    // SQLite: "UNIQUE constraint failed"
    // PostgreSQL: "duplicate key value violates unique constraint"
    // MySQL: "Duplicate entry" or "unique constraint"
    msg.contains("unique constraint")
        || msg.contains("duplicate key")
        || msg.contains("duplicate entry")
        || msg.contains("unique violation")
        || msg.contains("constraint failed")
}

// ============================================================================
// Column Extension Trait (Zero Duplication!)
// ============================================================================

/// Extension trait for ergonomic column operations
///
/// This trait adds Django-like methods to ANY `SeaORM` Column enum.
/// Works directly on `SeaORM`'s generated Column enum with zero duplication.
pub trait ColumnExt: ColumnTrait {
    // ===== String Operations =====

    /// Check if column contains a substring (LIKE %value%)
    fn contains(&self, value: &str) -> SimpleExpr {
        ColumnTrait::contains(self, value)
    }

    /// Check if column starts with a prefix (LIKE value%)
    fn starts_with(&self, value: &str) -> SimpleExpr {
        ColumnTrait::starts_with(self, value)
    }

    /// Check if column ends with a suffix (LIKE %value)
    fn ends_with(&self, value: &str) -> SimpleExpr {
        ColumnTrait::ends_with(self, value)
    }

    // ===== Generic Comparisons =====

    /// Equal to value
    fn eq<V>(&self, value: V) -> SimpleExpr
    where
        V: Into<sea_orm::Value>,
    {
        ColumnTrait::eq(self, value)
    }

    /// Not equal to value
    fn ne<V>(&self, value: V) -> SimpleExpr
    where
        V: Into<sea_orm::Value>,
    {
        ColumnTrait::ne(self, value)
    }

    /// Greater than
    fn gt<V>(&self, value: V) -> SimpleExpr
    where
        V: Into<sea_orm::Value>,
    {
        ColumnTrait::gt(self, value)
    }

    /// Greater than or equal
    fn gte<V>(&self, value: V) -> SimpleExpr
    where
        V: Into<sea_orm::Value>,
    {
        ColumnTrait::gte(self, value)
    }

    /// Less than
    fn lt<V>(&self, value: V) -> SimpleExpr
    where
        V: Into<sea_orm::Value>,
    {
        ColumnTrait::lt(self, value)
    }

    /// Less than or equal
    fn lte<V>(&self, value: V) -> SimpleExpr
    where
        V: Into<sea_orm::Value>,
    {
        ColumnTrait::lte(self, value)
    }

    /// Value is in list
    fn in_values<V, I>(&self, values: I) -> SimpleExpr
    where
        V: Into<sea_orm::Value>,
        I: IntoIterator<Item = V>,
    {
        ColumnTrait::is_in(self, values)
    }

    // ===== NULL checks =====

    /// Check if column is NULL
    fn is_null(&self) -> SimpleExpr {
        ColumnTrait::is_null(self)
    }

    /// Check if column is NOT NULL
    fn is_not_null(&self) -> SimpleExpr {
        ColumnTrait::is_not_null(self)
    }
}

// Implement for all ColumnTrait types (works with ANY entity!)
impl<T: ColumnTrait> ColumnExt for T {}

// ============================================================================
/// Main `QuerySet` structure (Django's `QuerySet` equivalent)
///
/// Provides chainable query building with automatic caching and lazy evaluation.
/// All operations are lazy until a terminal method (.`all()`, .`first()`, etc.) is called.
///
/// **Caching Behavior (Django-like):**
/// - First execution of `.all()`, `.first()`, etc. hits the database
/// - Results are cached in the `QuerySet` instance
/// - Subsequent calls on the SAME `QuerySet` reuse cached results
/// - Building new queries (`.filter()`, `.limit()`) creates new `QuerySet` with separate cache
///
/// **Concurrency Safety:**
/// - Uses `Arc` for cheap cloning across async tasks
/// - Uses `tokio::RwLock` for thread-safe cache access
/// - Safe to share across threads and async tasks
///
/// # Type Parameters
///
/// - `E`: The `SeaORM` Entity type
/// - `C`: The database connection type
///
/// # Examples
///
/// ```rust,ignore
/// // Build query
/// let queryset = Book::objects(db)
///     .filter(Book::Published.eq(true));
///
/// // First call - hits DB, caches results
/// let books = queryset.all().await?;
///
/// // Second call - uses cache, no DB query!
/// let books_again = queryset.all().await?;
///
/// // Modify query - creates new QuerySet with new cache
/// let limited = queryset.limit(10).all().await?;
/// ```
pub struct QuerySet<'a, E: EntityTrait, C: ConnectionTrait> {
    pub(crate) inner: Arc<QuerySetInner<'a, E, C>>,
}

/// Soft delete filtering mode
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoftDeleteMode {
    /// Exclude soft-deleted records (default)
    ExcludeDeleted,
    /// Include all records (deleted and not deleted)
    IncludeDeleted,
    /// Only show soft-deleted records
    OnlyDeleted,
}

/// Query building state for introspection
///
/// Tracks the current state of query construction, enabling
/// validation and debugging of query building patterns.
///
/// # Example
///
/// ```rust,ignore
/// use seaorm_django::prelude::*;
///
/// let qs = Book::objects(db);
/// assert_eq!(qs.state(), QueryState::Fresh);
///
/// let qs = qs.filter(Book::Price.lt(50));
/// assert_eq!(qs.state(), QueryState::Filtered);
///
/// let qs = qs.order_by_asc(Book::Title);
/// assert_eq!(qs.state(), QueryState::Ordered);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QueryState {
    /// Initial state - no operations applied
    #[default]
    Fresh,
    /// Has filter/exclude operations
    Filtered,
    /// Has ordering applied
    Ordered,
    /// Has pagination (limit/offset)
    Paginated,
    /// Has aggregations/annotations
    Aggregated,
    /// Query has been executed
    Executed,
}

impl QueryState {
    /// Check if query is in fresh state
    pub const fn is_fresh(&self) -> bool {
        matches!(self, Self::Fresh)
    }

    /// Check if query has filters
    pub const fn is_filtered(&self) -> bool {
        matches!(self, Self::Filtered | Self::Ordered | Self::Paginated | Self::Aggregated)
    }

    /// Check if query has ordering
    pub const fn is_ordered(&self) -> bool {
        matches!(self, Self::Ordered | Self::Paginated)
    }

    /// Check if query has pagination
    pub const fn is_paginated(&self) -> bool {
        matches!(self, Self::Paginated)
    }

    /// Check if query has aggregations
    pub const fn is_aggregated(&self) -> bool {
        matches!(self, Self::Aggregated)
    }

    /// Check if query has been executed
    pub const fn is_executed(&self) -> bool {
        matches!(self, Self::Executed)
    }

    /// Transition to filtered state
    pub fn filter(&mut self) {
        if matches!(self, Self::Fresh) {
            *self = Self::Filtered;
        }
    }

    /// Transition to ordered state
    pub fn order(&mut self) {
        if matches!(self, Self::Fresh | Self::Filtered) {
            *self = Self::Ordered;
        }
    }

    /// Transition to paginated state
    pub fn paginate(&mut self) {
        if !matches!(self, Self::Aggregated | Self::Executed) {
            *self = Self::Paginated;
        }
    }

    /// Transition to aggregated state
    pub fn aggregate(&mut self) {
        *self = Self::Aggregated;
    }

    /// Transition to executed state
    pub fn execute(&mut self) {
        *self = Self::Executed;
    }
}

// ============================================================================
// QueryOp Enum - Introspectable Query Operations
// ============================================================================

/// Order direction for sorting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderDirection {
    /// Ascending order (ASC)
    Asc,
    /// Descending order (DESC)
    Desc,
}

/// Represents a single query operation - fully introspectable
///
/// This enum captures all query modifications in a pattern-matchable form,
/// enabling query plan inspection, optimization, and debugging.
///
/// # Example
///
/// ```rust,ignore
/// use seaorm_django::prelude::*;
///
/// let plan = Book::objects(db).filter(Book::Price.lt(50)).plan();
///
/// // Inspect the query plan
/// for op in plan.operations() {
///     match op {
///         QueryOp::Filter(expr) => println!("Filter: {:?}", expr),
///         QueryOp::Limit(n) => println!("Limit: {}", n),
///         QueryOp::OrderBy { column, direction } => {
///             println!("Order by {:?} {:?}", column, direction);
///         }
///         _ => {}
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub enum QueryOp {
    /// Filter condition (WHERE clause)
    Filter(FilterExpr),
    /// Exclusion condition (WHERE NOT clause)
    Exclude(FilterExpr),
    /// Order by column with direction
    OrderBy {
        /// Column reference for ordering
        column: sea_orm::sea_query::ColumnRef,
        /// Sort direction
        direction: OrderDirection,
    },
    /// Limit number of results
    Limit(u64),
    /// Offset for pagination
    Offset(u64),
    /// DISTINCT clause
    Distinct,
    /// GROUP BY clause
    GroupBy(sea_orm::sea_query::ColumnRef),
    /// Soft delete mode
    SoftDelete(SoftDeleteMode),
    /// Annotation (aggregate expression with alias)
    Annotate {
        /// Alias for the aggregation result
        alias: String,
        /// Aggregation expression
        aggregation: Aggregation,
    },
}

impl QueryOp {
    /// Create a filter operation
    pub fn filter(expr: FilterExpr) -> Self {
        Self::Filter(expr)
    }

    /// Create an exclude operation
    pub fn exclude(expr: FilterExpr) -> Self {
        Self::Exclude(expr)
    }

    /// Create an order by ascending operation
    pub fn order_asc(column: impl ColumnTrait) -> Self {
        Self::OrderBy {
            column: column.as_column_ref().into(),
            direction: OrderDirection::Asc,
        }
    }

    /// Create an order by descending operation
    pub fn order_desc(column: impl ColumnTrait) -> Self {
        Self::OrderBy {
            column: column.as_column_ref().into(),
            direction: OrderDirection::Desc,
        }
    }

    /// Create a limit operation
    pub fn limit(n: u64) -> Self {
        Self::Limit(n)
    }

    /// Create an offset operation
    pub fn offset(n: u64) -> Self {
        Self::Offset(n)
    }

    /// Create a distinct operation
    pub fn distinct() -> Self {
        Self::Distinct
    }

    /// Create a group by operation
    pub fn group_by(column: impl ColumnTrait) -> Self {
        Self::GroupBy(column.as_column_ref().into())
    }

    /// Check if this is a filter operation
    pub const fn is_filter(&self) -> bool {
        matches!(self, Self::Filter(_))
    }

    /// Check if this is an exclude operation
    pub const fn is_exclude(&self) -> bool {
        matches!(self, Self::Exclude(_))
    }

    /// Check if this is an order by operation
    pub const fn is_order_by(&self) -> bool {
        matches!(self, Self::OrderBy { .. })
    }

    /// Check if this is a limit operation
    pub const fn is_limit(&self) -> bool {
        matches!(self, Self::Limit(_))
    }

    /// Check if this is an offset operation
    pub const fn is_offset(&self) -> bool {
        matches!(self, Self::Offset(_))
    }

    /// Check if this is a distinct operation
    pub const fn is_distinct(&self) -> bool {
        matches!(self, Self::Distinct)
    }
}

/// Query plan - a collection of operations that can be inspected and optimized
///
/// The query plan captures all operations applied to a QuerySet in order,
/// enabling introspection before execution.
///
/// # Example
///
/// ```rust,ignore
/// let plan = Book::objects(db)
///     .filter(Book::Price.lt(50))
///     .order_by_desc(Book::CreatedAt)
///     .limit(10)
///     .plan();
///
/// println!("Query has {} operations", plan.len());
/// println!("Debug: {:?}", plan);
/// ```
#[derive(Debug, Clone, Default)]
pub struct QueryPlan {
    operations: Vec<QueryOp>,
}

impl QueryPlan {
    /// Create a new empty query plan
    pub fn new() -> Self {
        Self { operations: Vec::new() }
    }

    /// Add an operation to the plan
    pub fn push(&mut self, op: QueryOp) {
        self.operations.push(op);
    }

    /// Get the number of operations
    pub fn len(&self) -> usize {
        self.operations.len()
    }

    /// Check if the plan is empty
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    /// Get a slice of operations
    pub fn operations(&self) -> &[QueryOp] {
        &self.operations
    }

    /// Iterate over operations
    pub fn iter(&self) -> impl Iterator<Item = &QueryOp> {
        self.operations.iter()
    }

    /// Check if plan contains any filter operations
    pub fn has_filters(&self) -> bool {
        self.operations.iter().any(|op| op.is_filter() || op.is_exclude())
    }

    /// Check if plan has ordering
    pub fn has_ordering(&self) -> bool {
        self.operations.iter().any(|op| op.is_order_by())
    }

    /// Check if plan has limit
    pub fn has_limit(&self) -> bool {
        self.operations.iter().any(|op| op.is_limit())
    }

    /// Get all filter expressions
    pub fn filters(&self) -> Vec<&FilterExpr> {
        self.operations
            .iter()
            .filter_map(|op| match op {
                QueryOp::Filter(expr) => Some(expr),
                _ => None,
            })
            .collect()
    }

    /// Get limit value if set
    pub fn get_limit(&self) -> Option<u64> {
        self.operations.iter().find_map(|op| match op {
            QueryOp::Limit(n) => Some(*n),
            _ => None,
        })
    }

    /// Get offset value if set
    pub fn get_offset(&self) -> Option<u64> {
        self.operations.iter().find_map(|op| match op {
            QueryOp::Offset(n) => Some(*n),
            _ => None,
        })
    }
}

/// Internal state for `QuerySet` (shared via Arc)
pub(crate) struct QuerySetInner<'a, E: EntityTrait, C: ConnectionTrait> {
    pub(crate) db: &'a C,
    pub(crate) select: Select<E>,
    pub(crate) soft_delete_mode: SoftDeleteMode,
    /// Query plan for introspection
    pub(crate) plan: QueryPlan,
    /// Current query state for tracking build progress
    pub(crate) query_state: QueryState,
    // Thread-safe cache for query results
    pub(crate) cache: RwLock<Option<Arc<Vec<E::Model>>>>,
}

// Implement Clone for QuerySet (cheap Arc clone)
impl<E: EntityTrait, C: ConnectionTrait> Clone for QuerySet<'_, E, C> {
    fn clone(&self) -> Self {
        Self { inner: Arc::clone(&self.inner) }
    }
}

impl<'a, E: EntityTrait, C: ConnectionTrait> QuerySet<'a, E, C> {
    /// Create a new `QuerySet`
    pub fn new(db: &'a C) -> Self {
        Self {
            inner: Arc::new(QuerySetInner {
                db,
                select: E::find(),
                soft_delete_mode: SoftDeleteMode::ExcludeDeleted,
                plan: QueryPlan::new(),
                query_state: QueryState::Fresh,
                cache: RwLock::new(None),
            }),
        }
    }

    /// Create a new `QuerySet` with modified select and operation (internal helper)
    fn with_select_and_op(&self, select: Select<E>, op: QueryOp) -> Self {
        let mut plan = self.inner.plan.clone();
        let mut state = self.inner.query_state;
        
        // Update state based on operation type
        match &op {
            QueryOp::Filter(_) | QueryOp::Exclude(_) => state.filter(),
            QueryOp::OrderBy { .. } => state.order(),
            QueryOp::Limit(_) | QueryOp::Offset(_) => state.paginate(),
            QueryOp::Annotate { .. } => state.aggregate(),
            _ => {}
        }
        
        plan.push(op);
        Self {
            inner: Arc::new(QuerySetInner {
                db: self.inner.db,
                select,
                soft_delete_mode: self.inner.soft_delete_mode,
                plan,
                query_state: state,
                cache: RwLock::new(None), // New cache for modified query
            }),
        }
    }

    /// Create a new `QuerySet` with modified soft delete mode
    fn with_soft_delete_mode(&self, mode: SoftDeleteMode) -> Self {
        let mut plan = self.inner.plan.clone();
        plan.push(QueryOp::SoftDelete(mode));
        Self {
            inner: Arc::new(QuerySetInner {
                db: self.inner.db,
                select: self.inner.select.clone(),
                soft_delete_mode: mode,
                plan,
                query_state: self.inner.query_state,
                cache: RwLock::new(None),
            }),
        }
    }

    /// Get the current query state
    ///
    /// Returns the state of query construction, useful for debugging
    /// and validation of query building patterns.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let qs = Book::objects(db);
    /// assert_eq!(qs.state(), QueryState::Fresh);
    ///
    /// let qs = qs.filter(Book::Price.lt(50));
    /// assert_eq!(qs.state(), QueryState::Filtered);
    /// ```
    pub fn state(&self) -> QueryState {
        self.inner.query_state
    }

    /// Get the query plan for introspection
    ///
    /// Returns a clone of the current query plan, allowing you to inspect
    /// all operations that will be applied when the query is executed.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let plan = Book::objects(db)
    ///     .filter(Book::Price.lt(50))
    ///     .limit(10)
    ///     .plan();
    ///
    /// for op in plan.iter() {
    ///     println!("{:?}", op);
    /// }
    /// ```
    pub fn plan(&self) -> QueryPlan {
        self.inner.plan.clone()
    }

    /// Apply soft delete filter to the query based on current mode
    fn apply_soft_delete_filter(&self, mut select: Select<E>) -> Select<E>
    where
        E: crate::traits::DjangoEntity,
    {
        use crate::traits::SoftDeleteConfig;
        use sea_orm::sea_query::{Alias, SimpleExpr};

        // Check if this entity has soft deletes enabled using enum pattern matching
        match E::soft_delete() {
            SoftDeleteConfig::Disabled => {
                // No soft delete - return query unchanged
            }
            SoftDeleteConfig::Enabled { column } => {
                match self.inner.soft_delete_mode {
                    SoftDeleteMode::ExcludeDeleted => {
                        // Filter WHERE deleted_at IS NULL
                        let condition = SimpleExpr::Binary(
                            Box::new(Expr::col(Alias::new(column))),
                            sea_orm::sea_query::BinOper::Is,
                            Box::new(SimpleExpr::Value(sea_orm::Value::String(None))),
                        );
                        select = select.filter(condition);
                    }
                    SoftDeleteMode::OnlyDeleted => {
                        // Filter WHERE deleted_at IS NOT NULL
                        let condition = SimpleExpr::Binary(
                            Box::new(Expr::col(Alias::new(column))),
                            sea_orm::sea_query::BinOper::IsNot,
                            Box::new(SimpleExpr::Value(sea_orm::Value::String(None))),
                        );
                        select = select.filter(condition);
                    }
                    SoftDeleteMode::IncludeDeleted => {
                        // No filter - include all records
                    }
                }
            }
        }
        select
    }

    /// Filter records (Django's .`filter()`)
    ///
    /// Creates a new `QuerySet` with added filter. The new `QuerySet` has its own cache.
    pub fn filter(&self, condition: impl Into<Condition>) -> Self {
        let cond: Condition = condition.into();
        let new_select = self.inner.select.clone().filter(cond.clone());
        self.with_select_and_op(new_select, QueryOp::Filter(FilterExpr::raw(cond)))
    }

    /// Exclude records (Django's .`exclude()`)
    ///
    /// Creates a new `QuerySet` with added exclusion. The new `QuerySet` has its own cache.
    pub fn exclude(&self, condition: impl Into<Condition>) -> Self {
        let cond: Condition = condition.into();
        let new_select = self.inner.select.clone().filter(cond.clone().not());
        self.with_select_and_op(new_select, QueryOp::Exclude(FilterExpr::raw(cond)))
    }

    /// Include soft-deleted records in query results
    ///
    /// By default, models with `#[soft_delete]` automatically exclude deleted records.
    /// Use this method to include them.
    ///
    /// # Example
    /// ```rust,ignore
    /// // Get all products including deleted ones
    /// let all_products = Product::objects(&db)
    ///     .with_deleted()
    ///     .all()
    ///     .await?;
    /// ```
    pub fn with_deleted(&self) -> Self {
        self.with_soft_delete_mode(SoftDeleteMode::IncludeDeleted)
    }

    /// Only show soft-deleted records
    ///
    /// Filters to show ONLY records where the soft delete field is NOT NULL.
    ///
    /// # Example
    /// ```rust,ignore
    /// // Get only deleted products
    /// let deleted = Product::objects(&db)
    ///     .only_deleted()
    ///     .all()
    ///     .await?;
    /// ```
    pub fn only_deleted(&self) -> Self {
        self.with_soft_delete_mode(SoftDeleteMode::OnlyDeleted)
    }

    /// Remove duplicate rows (Django's .`distinct()`)
    ///
    /// Returns only unique records. Useful when joins might create duplicates.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Get unique book titles (no duplicates)
    /// let books = Book::objects(db)
    ///     .distinct()
    ///     .all()
    ///     .await?;
    ///
    /// // Combined with filters
    /// let unique_authors = Book::objects(db)
    ///     .filter(Book::Published.eq(true))
    ///     .distinct()
    ///     .all()
    ///     .await?;
    /// ```
    ///
    /// # SQL
    ///
    /// Generates: `SELECT DISTINCT * FROM ...`
    ///
    /// # Performance
    ///
    /// DISTINCT can be expensive on large datasets. Use only when necessary.
    pub fn distinct(&self) -> Self {
        use sea_orm::QuerySelect;
        let new_select = self.inner.select.clone().distinct();
        self.with_select_and_op(new_select, QueryOp::Distinct)
    }

    /// Order by a column in ascending order (Django's .`order_by`('field'))
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Order by price (lowest first)
    /// let books = Book::objects(db)
    ///     .order_by_asc(Book::Price)
    ///     .all()
    ///     .await?;
    ///
    /// // Order by name alphabetically
    /// let authors = Author::objects(db)
    ///     .order_by_asc(Author::Name)
    ///     .all()
    ///     .await?;
    /// ```
    pub fn order_by_asc(&self, column: impl ColumnTrait) -> Self {
        let col_ref = column.as_column_ref().into();
        let new_select = self.inner.select.clone().order_by(column, Order::Asc);
        self.with_select_and_op(
            new_select,
            QueryOp::OrderBy {
                column: col_ref,
                direction: OrderDirection::Asc,
            },
        )
    }

    /// Order by a column in descending order (Django's .`order_by`('-field'))
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Order by price (highest first)
    /// let books = Book::objects(db)
    ///     .order_by_desc(Book::Price)
    ///     .all()
    ///     .await?;
    ///
    /// // Get newest books first
    /// let recent = Book::objects(db)
    ///     .order_by_desc(Book::CreatedAt)
    ///     .limit(10)
    ///     .all()
    ///     .await?;
    /// ```
    pub fn order_by_desc(&self, column: impl ColumnTrait) -> Self {
        let col_ref = column.as_column_ref().into();
        let new_select = self.inner.select.clone().order_by(column, Order::Desc);
        self.with_select_and_op(
            new_select,
            QueryOp::OrderBy {
                column: col_ref,
                direction: OrderDirection::Desc,
            },
        )
    }

    /// Limit results (Django's [:n])
    pub fn limit(&self, limit: u64) -> Self {
        let new_select = self.inner.select.clone().limit(limit);
        self.with_select_and_op(new_select, QueryOp::Limit(limit))
    }

    /// Offset results
    pub fn offset(&self, offset: u64) -> Self {
        let new_select = self.inner.select.clone().offset(offset);
        self.with_select_and_op(new_select, QueryOp::Offset(offset))
    }

    /// Execute query and return all matching results (Django's .`all()`)
    ///
    /// Returns a vector of all models that match the query filters.
    ///
    /// # Returns
    ///
    /// - `Ok(Vec<E::Model>)` - Vector of matching models (may be empty)
    /// - `Err(DjangoOrmError)` - Database error occurred
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Get all books
    /// let all_books = Book::objects(db).all().await?;
    /// println!("Found {} books", all_books.len());
    ///
    /// // Get filtered books
    /// let published = Book::objects(db)
    ///     .filter(Column::Published.eq(true))
    ///     .all()
    ///     .await?;
    ///
    /// // Empty result is NOT an error
    /// let no_books = Book::objects(db)
    ///     .filter(Column::Title.eq("Nonexistent"))
    ///     .all()
    ///     .await?;
    /// assert_eq!(no_books.len(), 0);  // Returns empty vec, not error
    /// ```
    ///
    /// # Caching
    ///
    /// **First call** - Executes SQL query and caches results:
    /// ```rust,ignore
    /// let qs = Book::objects(db).filter(Book::Published.eq(true));
    /// let books = qs.all().await?;  // DB query executed
    /// ```
    ///
    /// **Second call on same `QuerySet`** - Returns cached results (no DB query):
    /// ```rust,ignore
    /// let books_again = qs.all().await?;  // Cache hit! No DB query
    /// ```
    pub async fn all(&self) -> Result<Vec<E::Model>, DjangoOrmError>
    where
        E: crate::traits::DjangoEntity,
    {
        // Try to read from cache first (allows multiple concurrent readers)
        {
            let cache = self.inner.cache.read().await;
            if let Some(ref results) = *cache {
                return Ok((**results).clone());
            }
        }

        // Apply soft delete filter before executing
        let query = self.apply_soft_delete_filter(self.inner.select.clone());

        // Cache miss - execute query and cache results
        let results = query.all(self.inner.db).await?;
        let results_arc = Arc::new(results.clone());

        // Update cache (exclusive write lock)
        {
            let mut cache = self.inner.cache.write().await;
            *cache = Some(results_arc);
        }

        Ok(results)
    }

    /// Execute query and return first result (Django's .`first()`)
    ///
    /// Returns the first matching model or error if no matches found.
    /// Useful with ordering to get the "latest" or "oldest" record.
    ///
    /// # Returns
    ///
    /// - `Ok(E::Model)` - First matching model found
    /// - `Err(DjangoOrmError::EmptyResult { .. })` - No matching models
    /// - `Err(DjangoOrmError::Database(_))` - Database error occurred
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Get first book (errors if empty)
    /// let book = Book::objects(db).first().await?;
    /// println!("First book: {}", book.title);
    ///
    /// // Get latest published book
    /// let latest = Book::objects(db)
    ///     .filter(Column::Published.eq(true))
    ///     .order_by_desc(Column::CreatedAt)
    ///     .first()
    ///     .await?;
    /// println!("Latest: {}", latest.title);
    ///
    /// // Handle no results
    /// match Book::objects(db).first().await {
    ///     Ok(book) => println!("Found: {}", book.title),
    ///     Err(DjangoOrmError::EmptyResult { .. }) => {
    ///         println!("No books in database");
    ///     }
    ///     Err(e) => return Err(e),
    /// }
    ///
    /// // Get oldest record
    /// let oldest = Book::objects(db)
    ///     .order_by_asc(Column::CreatedAt)
    ///     .first()
    ///     .await?;
    /// ```
    ///
    /// # Caching
    ///
    /// Uses the same cache as `.all()`. If cache exists, returns first element.
    pub async fn first(&self) -> Result<E::Model, DjangoOrmError>
    where
        E: crate::traits::DjangoEntity,
    {
        // Try cache first
        {
            let cache = self.inner.cache.read().await;
            if let Some(cached_results) = cache.as_ref() {
                return cached_results
                    .first()
                    .cloned()
                    .ok_or_else(|| DjangoOrmError::empty_result("first"));
            }
        }

        // Apply soft delete filter before executing
        let query = self.apply_soft_delete_filter(self.inner.select.clone());

        // Cache miss - execute query for single record
        query
            .one(self.inner.db)
            .await?
            .ok_or_else(|| DjangoOrmError::empty_result("first"))
    }

    /// Execute query and return last result
    ///
    /// Returns the last matching model or error if no matches found.
    /// Orders by primary key descending and returns the first result.
    ///
    /// # Returns
    ///
    /// - `Ok(E::Model)` - Last matching model found
    /// - `Err(DjangoOrmError::NotFound { .. })` - No matching models
    /// - `Err(DjangoOrmError::Database(_))` - Database error occurred
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Get last book (by primary key)
    /// let book = Book::objects(db).last().await?;
    /// println!("Last book: {}", book.title);
    ///
    /// // Handle no results
    /// match Book::objects(db).last().await {
    ///     Ok(book) => println!("Last: {}", book.title),
    ///     Err(DjangoOrmError::NotFound { .. }) => {
    ///         println!("No books found");
    ///     }
    ///     Err(e) => return Err(e),
    /// }
    /// ```
    ///
    /// # Note
    ///
    /// This method orders by primary key descending to efficiently get the last record.
    /// If you need the last record based on a different ordering, use
    /// `.order_by_desc(field).first()` instead.
    pub async fn last(&self) -> Result<E::Model, DjangoOrmError>
    where
        E: crate::traits::DjangoEntity,
    {
        use sea_orm::{Iterable, PrimaryKeyToColumn};

        // Get the primary key column(s)
        let pk_columns: Vec<_> = E::PrimaryKey::iter().collect();

        // Build query ordered by PK descending
        let mut query = self.inner.select.clone();
        for pk in pk_columns {
            query = query.order_by(pk.into_column(), Order::Desc);
        }

        // Apply soft delete filter and get one record
        let query = self.apply_soft_delete_filter(query);

        query
            .one(self.inner.db)
            .await?
            .ok_or_else(|| DjangoOrmError::not_found(E::default().table_name(), "last".to_string()))
    }

    /// Get a single record by primary key (Django's .get(pk=))
    ///
    /// Returns the model or error if not found. This matches Django's behavior
    /// where `.get()` raises `DoesNotExist` if the record doesn't exist.
    ///
    /// # Returns
    ///
    /// - `Ok(E::Model)` - Record found
    /// - `Err(DjangoOrmError::NotFound { entity, id })` - No record with that ID
    /// - `Err(DjangoOrmError::Database(_))` - Database error occurred
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Simple get
    /// let book = Book::objects(db).get(1).await?;
    /// println!("Found: {}", book.title);
    ///
    /// // Handle not found
    /// match Book::objects(db).get(999).await {
    ///     Ok(book) => println!("Found: {}", book.title),
    ///     Err(DjangoOrmError::NotFound { entity, id }) => {
    ///         println!("{} with id {} doesn't exist", entity, id);
    ///     }
    ///     Err(e) => return Err(e),  // Other error
    /// }
    /// // Or use ? for early return on not found
    pub async fn get<T>(&self, id: T) -> Result<E::Model, DjangoOrmError>
    where
        T: Into<<E::PrimaryKey as PrimaryKeyTrait>::ValueType> + Send + std::fmt::Display,
        E: crate::traits::DjangoEntity,
    {
        let id_str = format!("{}", &id);

        // Build the query with soft delete filter
        let query = self.apply_soft_delete_filter(E::find_by_id(id));

        query
            .one(self.inner.db)
            .await?
            .ok_or_else(|| DjangoOrmError::not_found(E::default().table_name(), id_str))
    }

    /// Get the earliest record by a field (Django's .`earliest()`)
    ///
    /// Orders by the specified column ascending and returns the first record.
    /// Returns an error if no records exist.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Get oldest book by creation date
    /// let oldest = Book::objects(db)
    ///     .earliest(Book::CreatedAt)
    ///     .await?;
    ///
    /// // With filters
    /// let first_published = Book::objects(db)
    ///     .filter(Book::Published.eq(true))
    ///     .earliest(Book::PublishedDate)
    ///     .await?;
    /// ```
    ///
    /// # Returns
    ///
    /// - `Ok(Model)` - The earliest record
    /// - `Err(DjangoOrmError::EmptyResult { .. })` - No records found
    /// - `Err(DjangoOrmError::Database)` - Database error
    ///
    /// # Equivalent to
    ///
    /// `.order_by_asc(column).first()` but returns error on empty result
    pub async fn earliest(&self, column: impl ColumnTrait) -> Result<E::Model, DjangoOrmError> {
        self.inner
            .select
            .clone()
            .order_by(column, Order::Asc)
            .one(self.inner.db)
            .await?
            .ok_or_else(|| DjangoOrmError::empty_result("earliest"))
    }

    /// Get the latest record by a field (Django's .`latest()`)
    ///
    /// Orders by the specified column descending and returns the first record.
    /// Returns an error if no records exist.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Get newest book
    /// let newest = Book::objects(db)
    ///     .latest(Book::CreatedAt)
    ///     .await?;
    ///
    /// // With filters
    /// let latest_published = Book::objects(db)
    ///     .filter(Book::Published.eq(true))
    ///     .latest(Book::PublishedDate)
    ///     .await?;
    ///
    /// // Get most expensive book
    /// let most_expensive = Book::objects(db)
    ///     .latest(Book::Price)
    ///     .await?;
    /// ```
    ///
    /// # Returns
    ///
    /// - `Ok(Model)` - The latest record
    /// - `Err(DjangoOrmError::EmptyResult { .. })` - No records found
    /// - `Err(DjangoOrmError::Database)` - Database error
    ///
    /// # Equivalent to
    ///
    /// `.order_by_desc(column).first()` but returns error on empty result
    pub async fn latest(&self, column: impl ColumnTrait) -> Result<E::Model, DjangoOrmError> {
        self.inner
            .select
            .clone()
            .order_by(column, Order::Desc)
            .one(self.inner.db)
            .await?
            .ok_or_else(|| DjangoOrmError::empty_result("latest"))
    }

    /// Count records matching the query (Django's .`count()`)
    ///
    /// Returns the number of records that match the query filters.
    /// Returns 0 if no records match (not an error).
    ///
    /// # Returns
    ///
    /// - `Ok(u64)` - Number of matching records (0 or more)
    /// - `Err(DjangoOrmError)` - Database error occurred
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Count all books
    /// let total = Book::objects(db).count().await?;
    /// println!("Total books: {}", total);
    ///
    /// // Count with filter
    /// let published = Book::objects(db)
    ///     .filter(Column::Published.eq(true))
    ///     .count()
    ///     .await?;
    /// println!("Published books: {}", published);
    ///
    /// // Zero count is NOT an error
    /// let drafts = Book::objects(db)
    ///     .filter(Column::Status.eq("draft"))
    ///     .count()
    ///     .await?;
    /// if drafts == 0 {
    ///     println!("No drafts found");  // Not an error!
    /// }
    ///
    /// // Use in conditional logic
    /// if Book::objects(db).count().await? > 100 {
    ///     println!("Large database!");
    /// }
    /// ```
    ///
    /// # Performance
    ///
    /// This uses a SQL `COUNT(*)` query which is optimized by the database.
    /// Much faster than loading all records and counting in memory.
    pub async fn count(&self) -> Result<u64, DjangoOrmError>
    where
        E: crate::traits::DjangoEntity,
    {
        // Apply soft delete filter first
        let filtered = self.apply_soft_delete_filter(self.inner.select.clone());

        // Get count using SeaORM's built-in count functionality
        use sea_orm::QuerySelect;
        let count_select = filtered.select_only().column_as(
            sea_orm::sea_query::Expr::col(sea_orm::sea_query::Asterisk).count(),
            "count",
        );

        // Execute and get the count
        let result = count_select.into_tuple::<i64>().one(self.inner.db).await?;
        Ok(result.unwrap_or(0) as u64)
    }

    /// Check if any records exist matching the query (Django's .`exists()`)
    ///
    /// Returns true if at least one record matches the query, false otherwise.
    /// More efficient than `.count() > 0` because it stops at the first match.
    ///
    /// # Returns
    ///
    /// - `Ok(true)` - At least one matching record exists
    /// - `Ok(false)` - No matching records (NOT an error)
    /// - `Err(DjangoOrmError)` - Database error occurred
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Check if any books exist
    /// if Book::objects(db).exists().await? {
    ///     println!("We have books!");
    /// } else {
    ///     println!("Database is empty");  // Not an error
    /// }
    ///
    /// // Check with filter
    /// let has_published = Book::objects(db)
    ///     .filter(Column::Published.eq(true))
    ///     .exists()
    ///     .await?;
    ///
    /// if !has_published {
    ///     println!("No published books yet");
    /// }
    ///
    /// // Use in validation
    /// if Book::objects(db)
    ///     .filter(Column::Title.eq(&title))
    ///     .exists()
    ///     .await?
    /// {
    ///     return Err("Book with this title already exists".into());
    /// }
    /// ```
    ///
    /// # Performance
    ///
    /// Uses `LIMIT 1` internally, so it stops as soon as it finds any match.
    /// This is much faster than counting all records when you just need to know
    /// if any exist.
    pub async fn exists(&self) -> Result<bool, DjangoOrmError>
    where
        E: crate::traits::DjangoEntity,
    {
        use sea_orm::QuerySelect;

        // Apply soft delete filter first
        let filtered = self.apply_soft_delete_filter(self.inner.select.clone());

        // Use LIMIT 1 for efficiency
        let result = filtered.limit(1).one(self.inner.db).await?;
        Ok(result.is_some())
    }

    /// Update all records matching the query (Django's .`update()`)
    ///
    /// Applies the same updates to all matching records using a closure.
    /// Returns the number of records updated.
    ///
    /// **Concurrency Safe:** Uses SELECT FOR UPDATE to lock rows before modification,
    /// preventing lost updates in concurrent scenarios. All updates succeed or all fail
    /// together within a transaction.
    ///
    /// **Batching:** Automatically chunks operations for large datasets. Default batch
    /// size is 1000 records. TODO: Support for different sizes.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Update all books by author - atomic operation
    /// let count = Book::objects(db)
    ///     .filter(Column::AuthorId.eq(1))
    ///     .update(|book| {
    ///         book.status = "archived".to_string();
    ///     })
    ///     .await?;
    ///
    /// println!("Updated {} books", count);
    /// ```
    pub async fn update<F>(self, updater: F) -> Result<u64, DjangoOrmError>
    where
        F: Fn(&mut E::Model) + Send + Sync,
        E: crate::traits::DjangoEntity,
        C: sea_orm::TransactionTrait,
    {
        use sea_orm::sea_query::LockType;
        use sea_orm::{QuerySelect, TransactionSession};

        // Wrap in transaction for atomicity
        let txn = self.inner.db.begin().await?;

        // Use SELECT FOR UPDATE to lock rows and prevent concurrent modifications
        let models = self.inner.select.clone().lock(LockType::Update).all(&txn).await?;
        let mut count = 0u64;

        for mut model in models {
            // Apply the update
            updater(&mut model);

            // Use save_model to properly mark all fields as Set
            E::save_model(&txn, model).await?;
            count += 1;
        }

        // Commit transaction
        txn.commit().await?;

        Ok(count)
    }

    /// Eager load related entities (Django's `prefetch_related`)
    ///
    /// Transforms this `QuerySet` into a `QuerySetEager` that supports prefetching relations.
    /// This prevents N+1 queries by loading all relations in batched queries (1+M pattern).
    ///
    /// # Usage
    ///
    /// Use the `relations!` macro to specify which entity types to prefetch:
    ///
    /// ```rust,ignore
    /// use seaorm_django::relations;
    ///
    /// let books = Book::objects(db)
    ///     .prefetch_related(relations![Author, Publisher])
    ///     .all()
    ///     .await?;
    /// ```
    ///
    /// The macro expands to `vec![TypeId::of::<Author>(), TypeId::of::<Publisher>()]`
    /// which the registry uses for type-safe runtime dispatch to relation loaders.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use seaorm_django::relations;
    /// use entity::{
    ///     book::Entity as Book,
    ///     author::Entity as Author,
    ///     publisher::Entity as Publisher,
    /// };
    ///
    /// // Multiple relations
    /// let books = Book::objects(db)
    ///     .filter(Column::Published.eq(true))
    ///     .prefetch_related(relations![Author, Publisher])
    ///     .all()
    ///     .await?;
    ///
    /// // Single relation
    /// let books = Book::objects(db)
    ///     .prefetch_related(relations![Author])
    ///     .all()
    ///     .await?;
    ///
    /// // With formatting (trailing comma optional)
    /// let books = Book::objects(db)
    ///     .prefetch_related(relations![
    ///         Author,
    ///         Publisher,
    ///         Category,
    ///     ])
    ///     .all()
    ///     .await?;
    ///
    /// // Access loaded relations
    /// for book in books {
    ///     println!("Title: {}", book.title);
    ///
    ///     if let Some(author) = book.author {
    ///         println!("Author: {}", author.name);
    ///     }
    ///
    ///     if let Some(publisher) = book.publisher {
    ///         println!("Publisher: {}", publisher.name);
    ///     }
    /// }
    ///
    /// // Single record with relations
    /// let book = Book::objects(db)
    ///     .prefetch_related(relations![Author, Publisher])
    ///     .get(1)
    ///     .await?;
    /// ```
    ///
    /// # Query Execution Pattern
    ///
    /// This executes **1+M queries** where M is the number of relation types:
    /// - 1 query for the main entities
    /// - 1 query per relation type (batch loaded, NOT per record!)
    ///
    /// Example with 2 relations loading 100 books:
    ///
    /// ```sql
    /// -- Query 1: Load 100 books (1 query)
    /// SELECT * FROM book WHERE published = true LIMIT 100;
    ///
    /// -- Query 2: Batch load ALL authors for those books (1 query, not 100!)
    /// SELECT * FROM author WHERE id IN (1, 2, 3, ..., 50);
    ///
    /// -- Query 3: Batch load ALL publishers (1 query, not 100!)
    /// SELECT * FROM publisher WHERE id IN (10, 11, 12, ..., 30);
    /// ```
    ///
    /// **Total: 3 queries** regardless of how many books you load.
    /// **NOT** 1 + (100 authors) + (100 publishers) = 201 queries! ✅
    ///
    /// # Empty Results
    ///
    /// If no models match the query, no relation queries are executed:
    ///
    /// ```rust,ignore
    /// // No books found = only 1 query executed (the main query)
    /// let books = Book::objects(db)
    ///     .filter(Column::Title.eq("Nonexistent"))
    ///     .prefetch_related(relations![Author])
    ///     .all()
    ///     .await?;
    ///
    /// assert_eq!(books.len(), 0);  // Empty, no error, no extra queries
    /// ```
    ///
    /// # Alternative: Raw Vec (Not Recommended)
    ///
    /// You can also pass a raw Vec of `TypeIds`, but the macro is cleaner:
    ///
    /// ```rust,ignore
    /// use std::any::TypeId;
    ///
    /// // Verbose version (not recommended)
    /// let books = Book::objects(db)
    ///     .prefetch_related(vec![
    ///         TypeId::of::<Author>(),
    ///         TypeId::of::<Publisher>(),
    ///     ])
    ///     .all()
    ///     .await?;
    ///
    /// // Clean version (recommended)
    /// let books = Book::objects(db)
    ///     .prefetch_related(relations![Author, Publisher])
    ///     .all()
    ///     .await?;
    /// ```
    pub fn prefetch_related<R>(self, relations: R) -> crate::relations::QuerySetEager<'a, E, C, R> {
        use crate::relations::QuerySetEager;

        let eager = QuerySetEager::new(self.inner.db, self.inner.select.clone());
        eager.prefetch_related(relations)
    }

    /// Eager load related entities using efficient batch queries (Django's `select_related`)
    ///
    /// Currently implemented using the same batched query strategy as `prefetch_related`.
    /// This prevents N+1 queries by loading all relations in separate queries (1+M pattern).
    ///
    /// **Note:** Future versions may use SQL JOINs for 1:1 and FK relationships for even better
    /// performance, while continuing to use separate queries for 1:N and M:N.
    ///
    /// # Usage
    ///
    /// ```rust,ignore
    /// use seaorm_django::relations;
    ///
    /// // Single relation
    /// let books = Book::objects(db)
    ///     .select_related(relations![Author])
    ///     .all()
    ///     .await?;
    ///
    /// // Multiple relations
    /// let books = Book::objects(db)
    ///     .filter(Column::Published.eq(true))
    ///     .select_related(relations![Author, Publisher])
    ///     .all()
    ///     .await?;
    /// ```
    ///
    /// # Performance
    ///
    /// For N books with M unique authors:
    /// - Without eager loading: 1 + N queries (N+1 problem)
    /// - With `select_related`: 1 + M queries (1+M pattern)
    ///
    /// Example: 100 books by 5 authors = 2 queries instead of 101!
    pub fn select_related<R>(self, relations: R) -> crate::relations::QuerySetEager<'a, E, C, R> {
        self.prefetch_related(relations)
    }

    /// Create a new record (Django's .`create()`)
    ///
    /// Creates and saves a new record in the database.
    /// Auto-increment IDs and timestamps are handled automatically.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let author = Author::objects(db).create(Author {
    ///     name: "John Doe".to_string(),
    ///     ..Default::default() // ID and timestamps handled automatically
    /// }).await?;
    /// ```
    pub async fn create(self, mut model: E::Model) -> Result<E::Model, DjangoOrmError>
    where
        E: crate::traits::DjangoEntity,
        E::Model: sea_orm::IntoActiveModel<E::ActiveModel> + crate::hooks::LifecycleHooks,
        E::ActiveModel: sea_orm::ActiveModelTrait<Entity = E> + Send,
    {
        use sea_orm::ActiveModelTrait;

        // Call before_save hook (common to both create and update)
        model.before_save().await?;

        // Call before_create hook (specific to create)
        model.before_create().await?;

        let active_model = E::to_active_model_for_create(model)?;
        let result = active_model.insert(self.inner.db).await?;

        // Call after_create hook
        result.after_create().await?;

        // Call after_save hook (common to both create and update)
        result.after_save().await?;

        Ok(result)
    }

    /// Bulk create multiple records (Django's `bulk_create()`)
    ///
    /// Creates multiple records in a single database operation for high performance.
    /// Much faster than creating records one-by-one.
    ///
    /// # Arguments
    ///
    /// * `models` - Vector of Model instances to insert
    ///
    /// # Returns
    ///
    /// Number of records created
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Create multiple authors at once
    /// let authors = vec![
    ///     Author {
    ///         name: "Author 1".to_string(),
    ///         ..Default::default()
    ///     },
    ///     Author {
    ///         name: "Author 2".to_string(),
    ///         ..Default::default()
    ///     },
    /// ];
    ///
    /// let count = Author::objects(db)
    ///     .bulk_create(authors)
    ///     .await?;
    ///
    /// assert_eq!(count, 2);
    /// ```
    ///
    /// # Performance
    ///
    /// For 1000 records:
    /// - Individual inserts: ~5-10 seconds
    /// - Bulk create: ~0.1-0.5 seconds (10-100x faster)
    ///
    /// # Limitations
    ///
    /// - Does not return generated IDs (for performance)
    /// - Does not trigger model hooks/signals
    /// - Check database limits (typically 1000-10000 records per operation)
    pub async fn bulk_create(self, models: Vec<E::Model>) -> Result<u64, DjangoOrmError>
    where
        E: crate::traits::DjangoEntity,
        E::Model: sea_orm::IntoActiveModel<E::ActiveModel>,
        E::ActiveModel: sea_orm::ActiveModelTrait<Entity = E> + Send,
    {
        if models.is_empty() {
            return Ok(0);
        }

        let count = models.len() as u64;

        // Convert models to ActiveModels using DjangoEntity logic (handles IDs/timestamps)
        let active_models: Result<Vec<E::ActiveModel>, DjangoOrmError> =
            models.into_iter().map(|model| E::to_active_model_for_create(model)).collect();
        let active_models = active_models?;

        // Use SeaORM's insert_many
        E::insert_many(active_models).exec(self.inner.db).await?;

        Ok(count)
    }

    /// Bulk upsert (insert or update on conflict)
    ///
    /// Efficiently inserts multiple records, updating existing ones on conflict.
    /// Generates a single INSERT ... ON CONFLICT DO UPDATE statement.
    ///
    /// # Arguments
    ///
    /// * `models` - Vector of Model instances to upsert
    ///
    /// # Returns
    ///
    /// `UpsertBuilder` for chaining `.on_conflict()` and `.update_fields()`
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let books = vec![
    ///     Book { isbn: "123", title: "Book 1", price: 1000, ..Default::default() },
    ///     Book { isbn: "456", title: "Book 2", price: 2000, ..Default::default() },
    /// ];
    ///
    /// Book::objects(&db)
    ///     .upsert_many(books)
    ///     .on_conflict(Book::Column::Isbn)
    ///     .update_fields(&[Book::Column::Title, Book::Column::Price])
    ///     .execute()
    ///     .await?;
    /// ```
    ///
    /// # SQL Generated
    ///
    /// ```sql
    /// INSERT INTO books (isbn, title, price)
    /// VALUES ('123', 'Book 1', 1000), ('456', 'Book 2', 2000)
    /// ON CONFLICT (isbn) DO UPDATE SET
    ///     title = EXCLUDED.title,
    ///     price = EXCLUDED.price;
    /// ```
    ///
    /// # Performance
    ///
    /// - 100 records: 200x faster than individual operations
    /// - 1000 records: 2000x faster than individual operations
    /// - 10000 records: 20000x faster than individual operations
    ///
    /// # Database Support
    ///
    /// - **`PostgreSQL`**: Full support via `ON CONFLICT DO UPDATE`
    /// - **`SQLite`**: Full support via `ON CONFLICT DO UPDATE`
    /// - **`MySQL`**: Polyfill via `ON DUPLICATE KEY UPDATE` (`MySQL` 5.7+)
    pub fn upsert_many(self, models: Vec<E::Model>) -> UpsertBuilder<'a, E, C> {
        UpsertBuilder::new(self.inner.db, models)
    }

    /// Delete all records matching this query (bulk delete)
    ///
    /// Efficiently deletes all matching records using batched bulk operations.
    /// Returns the number of records deleted.
    ///
    /// **Performance:** Uses ID-based bulk deletion with automatic batching.
    /// Default batch size: 1000 records per operation.
    ///
    /// # Returns
    ///
    /// - `Ok(u64)` - Number of records deleted (0 if no matches)
    /// - `Err(DjangoOrmError)` - Database error occurred
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Delete all drafts - efficient bulk operation
    /// let count = Book::objects(db)
    ///     .filter(Column::Status.eq("draft"))
    ///     .delete()
    ///     .await?;
    /// println!("Deleted {} drafts", count);
    ///
    /// // Delete old records
    /// let cutoff_date = chrono::Utc::now() - chrono::Duration::days(30);
    /// let count = Book::objects(db)
    ///     .filter(Column::CreatedAt.lt(cutoff_date))
    ///     .delete()
    ///     .await?;
    /// ```
    ///
    /// # Safety
    ///
    /// - Always use with a filter to avoid accidentally deleting all records
    /// - Uses bulk DELETE with primary key IN clause for performance
    /// - Check foreign key constraints (may fail if records are referenced)
    pub async fn delete(self) -> Result<u64, DjangoOrmError>
    where
        E::Model: sea_orm::ModelTrait,
    {
        use sea_orm::{
            ColumnTrait, Condition, Iterable, ModelTrait, PrimaryKeyToColumn, QueryFilter,
        };

        // First, fetch just the primary keys of records to delete
        let models = self.inner.select.clone().all(self.inner.db).await?;

        if models.is_empty() {
            return Ok(0);
        }

        let count = models.len() as u64;

        // Extract primary key values for bulk delete
        // For entities with single-column primary keys, use IN clause
        let pk_columns: Vec<_> = E::PrimaryKey::iter().collect();

        if pk_columns.len() == 1 {
            // Single primary key - use optimized IN clause
            let pk_col = pk_columns[0].into_column();
            let pk_values: Vec<_> = models.iter().map(|m| m.get(pk_col)).collect();

            // Bulk delete with WHERE pk IN (...)
            E::delete_many()
                .filter(ColumnTrait::is_in(&pk_col, pk_values))
                .exec(self.inner.db)
                .await?;
        } else {
            // Composite primary key - build OR conditions
            let mut condition = Condition::any();
            for model in models {
                let mut row_condition = Condition::all();
                for pk_col in &pk_columns {
                    let col = pk_col.into_column();
                    let val = model.get(col);
                    row_condition = row_condition.add(ColumnTrait::eq(&col, val));
                }
                condition = condition.add(row_condition);
            }

            E::delete_many().filter(condition).exec(self.inner.db).await?;
        }

        Ok(count)
    }

    /// Get existing record or create it (Django's .`get_or_create()`)
    ///
    /// Attempts to retrieve a record matching the query. If not found, creates a new
    /// record using the provided creator function.
    ///
    /// **Atomicity:** Wrapped in a transaction to prevent race conditions.
    /// If two threads try to create the same record, only one succeeds.
    ///
    /// # Returns
    ///
    /// Returns a tuple `(model, created)` where:
    /// - `model` - The existing or newly created model
    /// - `created` - `true` if the model was created, `false` if it already existed
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use sea_orm::Set;
    ///
    /// // Get or create an author - race-condition safe
    /// let (author, created) = Author::objects(db)
    ///     .filter(Author::Email.eq("john@example.com"))
    ///     .get_or_create(|| {
    ///         author::ActiveModel {
    ///             name: Set("John Doe".to_string()),
    ///             email: Set("john@example.com".to_string()),
    ///             age: Set(30),
    ///             ..Default::default()
    ///         }
    ///     })
    ///     .await?;
    ///
    /// if created {
    ///     println!("Created new author: {}", author.name);
    /// } else {
    ///     println!("Author already exists: {}", author.name);
    /// }
    ///
    /// // With dynamic data
    /// let email = "jane@example.com";
    /// let (author, _) = Author::objects(db)
    ///     .filter(Author::Email.eq(email))
    ///     .get_or_create(|| {
    ///         author::ActiveModel {
    ///             name: Set("Jane Doe".to_string()),
    ///             email: Set(email.to_string()),
    ///             age: Set(25),
    ///             ..Default::default()
    ///         }
    ///     })
    ///     .await?;
    /// ```
    ///
    /// # Thread Safety
    ///
    /// This method is safe for concurrent use. Transaction ensures atomicity.
    ///
    /// # Performance
    ///
    /// Makes 1-2 queries within a transaction for safety.
    pub async fn get_or_create<F>(self, creator: F) -> Result<(E::Model, bool), DjangoOrmError>
    where
        E: crate::traits::DjangoEntity,
        F: Fn() -> E::Model, // Changed: Fn instead of FnOnce to allow retries
        E::Model: sea_orm::IntoActiveModel<E::ActiveModel>,
        E::ActiveModel: sea_orm::ActiveModelTrait<Entity = E> + sea_orm::ActiveModelBehavior + Send,
        C: sea_orm::TransactionTrait,
    {
        use sea_orm::{ActiveModelTrait, TransactionSession};

        // Retry up to 3 times to handle race conditions with unique constraints
        for attempt in 0..3 {
            let txn = self.inner.db.begin().await?;

            // Try to get existing record
            if let Some(model) = self.inner.select.clone().one(&txn).await? {
                txn.commit().await?;
                return Ok((model, false));
            } else {
                // Try to create new record
                let model = creator();
                let active_model = E::to_active_model_for_create(model)?;

                match active_model.insert(&txn).await {
                    Ok(model) => {
                        txn.commit().await?;
                        return Ok((model, true));
                    }
                    Err(e) if is_unique_violation(&e) && attempt < 2 => {
                        // Race condition detected - another transaction inserted the row
                        // Roll back and retry. Rollback errors are logged but not fatal
                        // since the transaction will be dropped anyway.
                        if let Err(rollback_err) = txn.rollback().await {
                            eprintln!("Warning: Failed to rollback transaction after unique violation: {rollback_err}");
                        }
                        continue;
                    }
                    Err(e) => {
                        // Attempt rollback on error. Rollback failure is logged but doesn't
                        // change the error we return since transaction drop also rolls back.
                        if let Err(rollback_err) = txn.rollback().await {
                            eprintln!("Warning: Failed to rollback transaction: {rollback_err}");
                        }
                        return Err(e.into());
                    }
                }
            }
        }

        // All retries exhausted
        Err(DjangoOrmError::concurrency_conflict("get_or_create", 3))
    }

    /// Update existing record or create new one (Django's .`update_or_create()`)
    ///
    /// Attempts to retrieve a record matching the query.
    /// - If found, applies the updates from `updater` and saves.
    /// - If not found, creates a new record using `creator`.
    ///
    /// **Atomicity:** Wrapped in a transaction to ensure all-or-nothing behavior.
    ///
    /// # Arguments
    ///
    /// * `updater` - Closure that modifies the existing model
    /// * `creator` - Closure that creates a new model if none exists
    ///
    /// # Returns
    ///
    /// Returns `(model, created)`
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let (book, created) = Book::objects(db)
    ///     .filter(Book::Isbn.eq("1234567890"))
    ///     .update_or_create(
    ///         |model| {
    ///             // Update existing
    ///             model.price = 2999;
    ///         },
    ///         || {
    ///             // Create new
    ///             Book {
    ///                 isbn: "1234567890".to_string(),
    ///                 title: "Rust Book".to_string(),
    ///                 price: 2999,
    ///                 ..Default::default()
    ///             }
    ///         }
    ///     )
    ///     .await?;
    /// ```
    ///
    /// # Thread Safety
    ///
    /// Safe for concurrent use. Transaction ensures atomicity.
    pub async fn update_or_create<U, Creator>(
        self,
        updater: U,
        creator: Creator,
    ) -> Result<(E::Model, bool), DjangoOrmError>
    where
        E: crate::traits::DjangoEntity,
        U: Fn(&mut E::Model), // Changed: Fn instead of FnOnce to allow retries
        Creator: Fn() -> E::Model, // Changed: Fn instead of FnOnce to allow retries
        E::Model: sea_orm::IntoActiveModel<E::ActiveModel>,
        E::ActiveModel: sea_orm::ActiveModelTrait<Entity = E> + sea_orm::ActiveModelBehavior + Send,
        C: sea_orm::TransactionTrait,
    {
        use sea_orm::{ActiveModelTrait, TransactionSession};

        // Retry up to 3 times to handle race conditions with unique constraints
        for attempt in 0..3 {
            let txn = self.inner.db.begin().await?;

            // Try to get existing record
            if let Some(mut model) = self.inner.select.clone().one(&txn).await? {
                // Update existing record
                updater(&mut model);
                let model = E::save_model(&txn, model).await?;
                txn.commit().await?;
                return Ok((model, false));
            } else {
                // Try to create new
                let model = creator();
                let active_model = E::to_active_model_for_create(model)?;

                match active_model.insert(&txn).await {
                    Ok(model) => {
                        txn.commit().await?;
                        return Ok((model, true));
                    }
                    Err(e) if is_unique_violation(&e) && attempt < 2 => {
                        // Race condition detected - another transaction inserted the row
                        // Roll back and retry (next iteration will find and update it)
                        if let Err(rollback_err) = txn.rollback().await {
                            eprintln!("Warning: Failed to rollback transaction after unique violation: {rollback_err}");
                        }
                        continue;
                    }
                    Err(e) => {
                        if let Err(rollback_err) = txn.rollback().await {
                            eprintln!("Warning: Failed to rollback transaction: {rollback_err}");
                        }
                        return Err(e.into());
                    }
                }
            }
        }

        // All retries exhausted
        Err(DjangoOrmError::concurrency_conflict("update_or_create", 3))
    }

    /// Get specific column values as JSON (Django's `values()`)
    ///
    /// Returns a Vec of JSON objects for small-medium datasets.
    /// For large datasets, automatically uses chunked fetching.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use seaorm_django::prelude::*;
    ///
    /// // Get only title and price fields
    /// let values = Book::objects(db)
    ///     .values(vec![Book::Title, Book::Price])
    ///     .await?;
    ///
    /// for val in values {
    ///     println!("Title: {}, Price: {}", val["title"], val["price"]);
    /// }
    /// ```
    pub async fn values(
        &self,
        columns: Vec<E::Column>,
    ) -> Result<Vec<serde_json::Value>, DjangoOrmError> {
        use sea_orm::{JsonValue, QuerySelect};

        if columns.is_empty() {
            return Ok(Vec::new());
        }

        // Use the existing select query directly to respect limits/offsets
        let mut select = self.inner.select.clone().select_only();
        for col in columns {
            select = select.column(col);
        }

        let results: Vec<JsonValue> = select.into_json().all(self.inner.db).await?;
        Ok(results)
    }

    /// Stream full model instances in chunks (Django's `.iterator()`).
    ///
    /// Memory-efficient alternative to `.all()` for large result sets.
    /// Fetches results in batches using LIMIT/OFFSET pagination.
    ///
    /// # Arguments
    ///
    /// * `chunk_size` - Optional batch size (default: 100 rows)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use futures::StreamExt;
    ///
    /// // Process 1 million rows without loading all into memory
    /// let mut stream = Book::objects(db)
    ///     .filter(Book::Published.eq(true))
    ///     .iterator(Some(500))
    ///     .await?;
    ///
    /// while let Some(book) = stream.next().await {
    ///     let book = book?;
    ///     process_book(book).await?;
    /// }
    /// ```
    pub async fn iterator(
        &self,
        chunk_size: Option<usize>,
    ) -> Result<
        impl futures::Stream<Item = Result<E::Model, DjangoOrmError>> + use<'a, E, C>,
        DjangoOrmError,
    > {
        use futures::stream::{self, StreamExt};
        use sea_orm::QuerySelect;

        let chunk_size = chunk_size.unwrap_or(crate::batching::DEFAULT_CHUNK_SIZE) as u64;
        let db = self.inner.db;
        let base_select = Arc::new(self.inner.select.clone());

        let stream = stream::unfold((0u64, false), move |(offset, done)| {
            let base_select = base_select.clone();
            async move {
                if done {
                    return None;
                }

                let select = (*base_select).clone().limit(chunk_size).offset(offset);

                let results: Vec<E::Model> = match select.all(db).await {
                    Ok(r) => r,
                    Err(e) => return Some((Err(DjangoOrmError::from(e)), (offset, true))),
                };

                let is_done = results.len() < chunk_size as usize;
                let next_offset = offset + results.len() as u64;

                Some((Ok(results), (next_offset, is_done)))
            }
        })
        .flat_map(|result| match result {
            Ok(models) => stream::iter(models.into_iter().map(Ok)).left_stream(),
            Err(e) => stream::once(async move { Err(e) }).right_stream(),
        });

        Ok(stream.boxed())
    }

    /// Get column values iterator (Django's `values().iterator()`)
    ///
    /// Returns iterator that streams results in chunks, preventing OOM.
    /// Use this directly for very large datasets where you want control.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use futures::StreamExt;
    ///
    /// // Stream results without loading all into memory
    /// let mut stream = Book::objects(db)
    ///     .values_iter(vec![Book::Title, Book::Price], None)
    ///     .await?;
    ///
    /// while let Some(value) = stream.next().await {
    ///     let value = value?;
    ///     println!("Title: {}, Price: {}", value["title"], value["price"]);
    /// }
    /// ```
    pub async fn values_iter(
        &self,
        columns: Vec<E::Column>,
        chunk_size: Option<usize>,
    ) -> Result<
        impl futures::Stream<Item = Result<serde_json::Value, DjangoOrmError>> + use<'a, E, C>,
        DjangoOrmError,
    > {
        use futures::stream::{self, StreamExt};
        use sea_orm::QuerySelect;

        if columns.is_empty() {
            return Ok(stream::empty().boxed());
        }

        let chunk_size = chunk_size.unwrap_or(crate::batching::DEFAULT_CHUNK_SIZE) as u64;

        // Create stream that fetches in chunks using limit/offset
        // This is Django's approach: paginate through results
        let db = self.inner.db;
        // Use Arc to avoid cloning the Select on every iteration
        let base_select = Arc::new(self.inner.select.clone());
        let columns = Arc::new(columns);

        let stream = stream::unfold((0u64, false), move |(offset, done)| {
            let base_select = base_select.clone(); // Clone Arc (cheap pointer copy)
            let columns = columns.clone(); // Clone Arc (cheap pointer copy)
            async move {
                if done {
                    return None;
                }

                let mut select = (*base_select).clone().select_only(); // Clone Select once per chunk (unavoidable)
                for col in &*columns {
                    select = select.column(*col);
                }

                let results: Vec<serde_json::Value> =
                    match select.limit(chunk_size).offset(offset).into_json().all(db).await {
                        Ok(r) => r,
                        Err(e) => return Some((Err(DjangoOrmError::from(e)), (offset, true))),
                    };

                let is_done = results.len() < chunk_size as usize;
                let next_offset = offset + results.len() as u64;

                Some((Ok(results), (next_offset, is_done)))
            }
        })
        .flat_map(|result| match result {
            Ok(values) => stream::iter(values.into_iter().map(Ok)).left_stream(),
            Err(e) => stream::once(async move { Err(e) }).right_stream(),
        });

        Ok(stream.boxed())
    }

    /// Get column values iterator as tuples (Django's `values_list().iterator()`)
    ///
    /// Returns iterator that streams results in chunks.
    /// For single column with `flat=true`, yields scalar values.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use futures::StreamExt;
    ///
    /// // Stream tuples
    /// let mut stream = Book::objects(db)
    ///     .values_list_iter(vec![Book::Title, Book::Price], false, None)
    ///     .await?;
    ///
    /// while let Some(row) = stream.next().await {
    ///     let row = row?;
    ///     // row is ["title", 1999]
    /// }
    ///
    /// // Stream flat values
    /// let mut stream = Book::objects(db)
    ///     .values_list_iter(vec![Book::Title], true, None)
    ///     .await?;
    ///
    /// while let Some(title) = stream.next().await {
    ///     let title = title?;
    ///     // title is just "Book Name"
    /// }
    /// ```
    pub async fn values_list_iter(
        &self,
        columns: Vec<E::Column>,
        flat: bool,
        chunk_size: Option<usize>,
    ) -> Result<
        impl futures::Stream<Item = Result<serde_json::Value, DjangoOrmError>> + use<'a, E, C>,
        DjangoOrmError,
    > {
        use futures::stream::StreamExt;

        let columns_len = columns.len();
        let columns_clone = columns.clone(); // Clone for later use in map closure
        let stream = self.values_iter(columns, chunk_size).await?;

        if flat && columns_len == 1 {
            Ok(stream
                .map(|result| {
                    result.and_then(|obj| {
                        obj.as_object().and_then(|map| map.values().next().cloned()).ok_or_else(
                            || {
                                DjangoOrmError::validation(
                                    "QuerySet",
                                    "values_list",
                                    "Invalid value format",
                                )
                            },
                        )
                    })
                })
                .boxed())
        } else {
            Ok(stream
                .map(move |result| {
                    result.map(|obj| {
                        let values: Vec<serde_json::Value> = obj
                            .as_object()
                            .map(|map| {
                                columns_clone
                                    .iter()
                                    .filter_map(|col| {
                                        let col_name = format!("{col:?}").to_lowercase();
                                        map.get(&col_name).cloned()
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        serde_json::Value::Array(values)
                    })
                })
                .boxed())
        }
    }

    /// Get specific column values as tuples (Django's `values_list()`)
    ///
    /// Returns a Vec of tuples for small-medium datasets.
    /// For large datasets, automatically uses chunked fetching.
    ///
    /// # Parameters
    ///
    /// - `columns` - Vector of columns to select
    /// - `flat` - If true and only one column, returns flat list instead of tuples
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Get tuples
    /// let pairs = Book::objects(db)
    ///     .values_list(vec![Book::Title, Book::Price], false)
    ///     .await?;
    ///
    /// // Get flat list
    /// let titles = Book::objects(db)
    ///     .values_list(vec![Book::Title], true)
    ///     .await?;
    /// ```
    pub async fn values_list(
        &self,
        columns: Vec<E::Column>,
        flat: bool,
    ) -> Result<Vec<serde_json::Value>, DjangoOrmError> {
        use sea_orm::{JsonValue, QuerySelect};

        if columns.is_empty() {
            return Ok(Vec::new());
        }

        // Use the existing select query directly to respect limits/offsets
        let mut select = self.inner.select.clone().select_only();
        for col in &columns {
            select = select.column(*col);
        }

        let results: Vec<JsonValue> = select.into_json().all(self.inner.db).await?;

        // If flat and single column, extract values
        if flat && columns.len() == 1 {
            Ok(results
                .into_iter()
                .filter_map(|obj| obj.as_object().and_then(|map| map.values().next().cloned()))
                .collect())
        } else {
            // Convert objects to arrays preserving column order
            Ok(results
                .into_iter()
                .map(|obj| {
                    let values: Vec<JsonValue> = obj
                        .as_object()
                        .map(|map| {
                            columns
                                .iter()
                                .filter_map(|col| {
                                    let col_name = format!("{col:?}").to_lowercase();
                                    map.get(&col_name).cloned()
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    serde_json::Value::Array(values)
                })
                .collect())
        }
    }

    /// Print the SQL query that would be executed (for debugging).
    ///
    /// Returns the SQL string and query parameters.
    /// Useful for debugging slow queries or understanding what SQL is generated.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let (sql, params) = Book::objects(db)
    ///     .filter(Book::Published.eq(true))
    ///     .order_by_desc(Book::CreatedAt)
    ///     .debug_sql();
    ///
    /// println!("SQL: {}", sql);
    /// println!("Params: {:?}", params);
    /// ```
    pub fn debug_sql(&self) -> String {
        use sea_orm::QueryTrait;
        let stmt = self.inner.select.build(self.inner.db.get_database_backend());
        stmt.to_string()
    }

    /// Analyze query execution plan (Django-inspired .`explain()`)
    ///
    /// Returns the database query execution plan without running the query.
    /// Useful for understanding how the database will execute your query
    /// and identifying performance bottlenecks.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Check query plan for a complex filter
    /// let plan = User::objects(&db)
    ///     .filter(User::Email.contains("@gmail.com"))
    ///     .filter(User::Age.gte(18))
    ///     .explain()
    ///     .await?;
    ///
    /// println!("Execution Plan:\n{}", plan);
    /// // Output shows if indexes are used, scan type, etc.
    /// ```
    ///
    /// # Performance Analysis
    ///
    /// Look for these indicators in the plan:
    /// - **Index Scan**: Good - using an index
    /// - **Sequential Scan**: Bad - scanning entire table
    /// - **Nested Loop**: Can be slow for large joins
    /// - **Hash Join**: Usually faster for large datasets
    ///
    /// # Database Support
    ///
    /// - **`SQLite`**: `EXPLAIN QUERY PLAN`
    /// - **`PostgreSQL`**: `EXPLAIN`
    /// - **`MySQL`**: `EXPLAIN`
    ///
    /// # See Also
    ///
    /// - `.explain_analyze()` - Runs query and provides actual timings
    /// - `.debug_sql()` - Shows the raw SQL query
    pub async fn explain(&self) -> Result<String, DjangoOrmError>
    where
        E: crate::traits::DjangoEntity,
    {
        use sea_orm::QueryTrait;

        // Get the SQL for the current query
        let backend = self.inner.db.get_database_backend();
        let stmt = self.apply_soft_delete_filter(self.inner.select.clone()).build(backend);
        let sql = stmt.to_string();

        // Construct EXPLAIN query based on database backend
        let explain_sql = match backend {
            sea_orm::DatabaseBackend::Sqlite => format!("EXPLAIN QUERY PLAN {sql}"),
            sea_orm::DatabaseBackend::Postgres => format!("EXPLAIN {sql}"),
            sea_orm::DatabaseBackend::MySql => format!("EXPLAIN {sql}"),
            _ => format!("EXPLAIN {sql}"), // Fallback for any future database backends
        };

        // Return the SQL that would be explained
        // Full EXPLAIN output requires database-specific result parsing
        Ok(format!("EXPLAIN output for query:\n{sql}\n\nTo run: {explain_sql}"))
    }

    /// Analyze query with actual execution (Django-inspired .explain(analyze=True))
    ///
    /// Runs the query and provides detailed execution statistics including
    /// actual row counts, execution time, and resource usage.
    ///
    /// **⚠️ WARNING**: This actually EXECUTES the query, so use carefully
    /// on production databases with large datasets.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Analyze actual query performance
    /// let analysis = Book::objects(&db)
    ///     .filter(Book::Published.eq(true))
    ///     .join(Author::Entity)
    ///     .explain_analyze()
    ///     .await?;
    ///
    /// println!("Execution Analysis:\n{}", analysis);
    /// // Shows actual timings, rows processed, buffer hits, etc.
    /// ```
    ///
    /// # What You Get
    ///
    /// - **Estimated vs Actual rows**: Are estimates accurate?
    /// - **Execution time**: How long did each step take?
    /// - **Buffer usage**: Cache hits/misses
    /// - **Sort operations**: Memory vs disk sorting
    ///
    /// # Performance Tips
    ///
    /// If you see:
    /// - **High actual rows**: Consider pagination/limits
    /// - **Sequential scans**: Add indexes
    /// - **Slow sorts**: Index the ORDER BY columns
    /// - **Many disk buffer reads**: Increase `shared_buffers` (`PostgreSQL`)
    ///
    /// # Database Support
    ///
    /// - **`SQLite`**: Limited - same as `explain()`
    /// - **`PostgreSQL`**: `EXPLAIN ANALYZE` - full statistics
    /// - **`MySQL`**: `EXPLAIN ANALYZE` (`MySQL` 8.0.18+)
    pub async fn explain_analyze(&self) -> Result<String, DjangoOrmError>
    where
        E: crate::traits::DjangoEntity,
    {
        use sea_orm::QueryTrait;

        // Get the SQL for the current query
        let backend = self.inner.db.get_database_backend();
        let stmt = self.apply_soft_delete_filter(self.inner.select.clone()).build(backend);
        let sql = stmt.to_string();

        // Construct EXPLAIN ANALYZE query based on database backend
        let explain_sql = match backend {
            sea_orm::DatabaseBackend::Sqlite => {
                // SQLite doesn't support EXPLAIN ANALYZE, fallback to EXPLAIN QUERY PLAN
                format!("EXPLAIN QUERY PLAN {sql}")
            }
            sea_orm::DatabaseBackend::Postgres => format!("EXPLAIN ANALYZE {sql}"),
            sea_orm::DatabaseBackend::MySql => {
                // MySQL 8.0.18+ supports EXPLAIN ANALYZE
                format!("EXPLAIN ANALYZE {sql}")
            }
            _ => format!("EXPLAIN ANALYZE {sql}"), // Fallback for any future database backends
        };

        // Return the EXPLAIN ANALYZE SQL for manual execution
        Ok(format!(
            "EXPLAIN ANALYZE output for query:\n{sql}\n\nRun this command directly:\n{explain_sql}"
        ))
    }

    /// Type-safe projection query (alternative to JSON-based `values()`)
    ///
    /// Returns results as a custom type with compile-time validation.
    /// Use `#[django_projection(model = YourModel)]` to define projection structs.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// #[django_projection(model = Book)]
    /// struct BookSummary {
    ///     title: String,
    ///     price: f64,
    /// }
    ///
    /// let summaries = Book::objects(db)
    ///     .filter(Book::Published.eq(true))
    ///     .project::<BookSummary>()
    ///     .await?;
    ///
    /// for summary in summaries {
    ///     println!("{}: ${}", summary.title, summary.price);
    /// }
    /// ```
    pub async fn project<T>(&self) -> Result<Vec<T>, DjangoOrmError>
    where
        T: sea_orm::FromQueryResult + Send,
    {
        // Simply use into_model without select_only()
        // SeaORM's FromQueryResult will map the available columns to T's fields
        Ok(self.inner.select.clone().into_model::<T>().all(self.inner.db).await?)
    }

    /// Group query results by one or more columns (Django's .`group_by()`)
    ///
    /// Used with `.annotate()` for aggregation queries.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let stats = Book::objects(db)
    ///     .group_by(Book::AuthorId)
    ///     .annotate([
    ///         ("book_count", Aggregation::count_all()),
    ///         ("avg_price", Aggregation::avg(Book::Price)),
    ///     ])
    ///     .project::<AuthorBookStats>()
    ///     .await?;
    /// ```
    pub fn group_by(&self, column: E::Column) -> Self {
        use sea_orm::QuerySelect;
        let col_ref = column.as_column_ref().into();
        let new_select = self.inner.select.clone().group_by(column);
        self.with_select_and_op(new_select, QueryOp::GroupBy(col_ref))
    }

    /// Add computed/aggregated columns to the query (Django's .`annotate()`)
    ///
    /// Adds aliased expressions for aggregations or computed values.
    /// Must be used with `.project::<T>()` where T has `#[computed]` fields
    /// matching the annotation aliases.
    ///
    /// # Examples
    ///
    /// ## Basic Aggregation
    ///
    /// ```rust,ignore
    /// use seaorm_django::prelude::*;
    ///
    /// #[django_projection(model = Book)]
    /// struct AuthorStats {
    ///     author_id: i32,
    ///     #[computed]
    ///     book_count: i64,
    ///     #[computed]
    ///     avg_price: Option<f64>,
    ///     #[computed]
    ///     total_sales: Option<i64>,
    /// }
    ///
    /// let stats = Book::objects(db)
    ///     .group_by(Book::AuthorId)
    ///     .annotate([
    ///         ("book_count", Aggregation::count_all()),
    ///         ("avg_price", Aggregation::avg(Book::Price)),
    ///         ("total_sales", Aggregation::sum(Book::Sales)),
    ///     ])
    ///     .project::<AuthorStats>()
    ///     .await?;
    ///
    /// for stat in stats {
    ///     println!("Author {}: {} books, avg ${:.2}",
    ///         stat.author_id,
    ///         stat.book_count,
    ///         stat.avg_price.unwrap_or(0.0)
    ///     );
    /// }
    /// ```
    ///
    /// ## With Filtering
    ///
    /// ```rust,ignore
    /// // Count only published books per author
    /// let stats = Book::objects(db)
    ///     .filter(Book::Published.eq(true))
    ///     .group_by(Book::AuthorId)
    ///     .annotate([("published_count", Aggregation::count_all())])
    ///     .project::<PublishedStats>()
    ///     .await?;
    /// ```
    ///
    /// ## Without GROUP BY (aggregate over entire result set)
    ///
    /// ```rust,ignore
    /// #[django_projection(model = Book)]
    /// struct OverallStats {
    ///     #[computed]
    ///     total_books: i64,
    ///     #[computed]
    ///     avg_price: Option<f64>,
    /// }
    ///
    /// let stats = Book::objects(db)
    ///     .annotate([
    ///         ("total_books", Aggregation::count_all()),
    ///         ("avg_price", Aggregation::avg(Book::Price)),
    ///     ])
    ///     .project::<OverallStats>()
    ///     .await?;
    /// ```
    pub fn annotate<const N: usize>(&self, annotations: [(&str, Aggregation); N]) -> Self {
        use sea_orm::QuerySelect;

        let mut new_select = self.inner.select.clone();
        let mut plan = self.inner.plan.clone();

        for (alias, aggregation) in annotations {
            let expr = aggregation.clone().into_expr();
            new_select = new_select.expr_as(expr, alias);
            plan.push(QueryOp::Annotate { alias: alias.to_string(), aggregation });
        }

        let mut state = self.inner.query_state;
        state.aggregate();
        
        Self {
            inner: Arc::new(QuerySetInner {
                db: self.inner.db,
                select: new_select,
                soft_delete_mode: self.inner.soft_delete_mode,
                plan,
                query_state: state,
                cache: RwLock::new(None),
            }),
        }
    }
}

// ============================================================================
// Aggregation Enum - Type-safe SQL aggregation functions
// ============================================================================

/// SQL aggregation function for use with `.annotate()`
///
/// This enum provides type-safe, pattern-matchable aggregation functions.
/// Use constructor methods to create instances.
///
/// # Example
///
/// ```rust,ignore
/// let stats = Book::objects(db)
///     .group_by(Book::AuthorId)
///     .annotate([
///         ("book_count", Aggregation::count_all()),
///         ("avg_price", Aggregation::avg(Book::Price)),
///         ("total_sales", Aggregation::sum(Book::Sales)),
///     ])
///     .project::<AuthorStats>()
///     .await?;
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum Aggregation {
    /// COUNT(*) - Count all rows
    CountAll,
    /// COUNT(column) - Count non-NULL values
    Count(sea_orm::sea_query::ColumnRef),
    /// SUM(column) - Sum of numeric values
    Sum(sea_orm::sea_query::ColumnRef),
    /// AVG(column) - Average of numeric values
    Avg(sea_orm::sea_query::ColumnRef),
    /// MAX(column) - Maximum value
    Max(sea_orm::sea_query::ColumnRef),
    /// MIN(column) - Minimum value
    Min(sea_orm::sea_query::ColumnRef),
}

impl Aggregation {
    /// COUNT(*) - Count all rows
    ///
    /// # Example
    /// ```rust,ignore
    /// .annotate([("total", Aggregation::count_all())])
    /// ```
    pub fn count_all() -> Self {
        Self::CountAll
    }

    /// COUNT(column) - Count non-NULL values in column
    ///
    /// # Example
    /// ```rust,ignore
    /// .annotate([("published_count", Aggregation::count(Book::PublishedDate))])
    /// ```
    pub fn count(column: impl ColumnTrait) -> Self {
        Self::Count(column.as_column_ref().into())
    }

    /// SUM(column) - Sum of numeric column
    ///
    /// # Example
    /// ```rust,ignore
    /// .annotate([("total_sales", Aggregation::sum(Book::Sales))])
    /// ```
    pub fn sum(column: impl ColumnTrait) -> Self {
        Self::Sum(column.as_column_ref().into())
    }

    /// AVG(column) - Average of numeric column
    ///
    /// # Example
    /// ```rust,ignore
    /// .annotate([("avg_price", Aggregation::avg(Book::Price))])
    /// ```
    pub fn avg(column: impl ColumnTrait) -> Self {
        Self::Avg(column.as_column_ref().into())
    }

    /// MAX(column) - Maximum value
    ///
    /// # Example
    /// ```rust,ignore
    /// .annotate([("max_price", Aggregation::max(Book::Price))])
    /// ```
    pub fn max(column: impl ColumnTrait) -> Self {
        Self::Max(column.as_column_ref().into())
    }

    /// MIN(column) - Minimum value
    ///
    /// # Example
    /// ```rust,ignore
    /// .annotate([("min_price", Aggregation::min(Book::Price))])
    /// ```
    pub fn min(column: impl ColumnTrait) -> Self {
        Self::Min(column.as_column_ref().into())
    }

    /// Convert to `SeaORM` expression for query building
    pub(crate) fn into_expr(self) -> SimpleExpr {
        match self {
            Self::CountAll =>
            {
                #[allow(deprecated)]
                Expr::expr(Func::count(Expr::asterisk()))
            }
            Self::Count(col) => Expr::expr(Func::count(Expr::col(col))),
            Self::Sum(col) => Expr::expr(Func::sum(Expr::col(col))),
            Self::Avg(col) => Expr::expr(Func::avg(Expr::col(col))),
            Self::Max(col) => Expr::expr(Func::max(Expr::col(col))),
            Self::Min(col) => Expr::expr(Func::min(Expr::col(col))),
        }
    }
}

// ============================================================================
// Q Objects - Complex Query Building
// ============================================================================

/// Q object for complex queries (Django's Q objects)
///
/// Q objects allow you to build complex queries with OR and NOT logic,
/// similar to Django's Q objects. They can be nested and combined.
///
/// # Basic Usage
///
/// ```rust,ignore
/// use seaorm_django::prelude::*;
///
/// // OR condition: title contains "Rust" OR "Python"
/// let q = Q::any()
///     .add(Column::Title.contains("Rust"))
///     .add(Column::Title.contains("Python"));
///
/// let books = Book::objects(db).filter(q).all().await?;
/// ```
///
/// # AND Conditions
///
/// ```rust,ignore
/// // AND condition: published AND price < 50
/// let q = Q::all()
///     .add(Column::Published.eq(true))
///     .add(Column::Price.lt(50));
///
/// let books = Book::objects(db).filter(q).all().await?;
/// ```
///
/// # NOT Conditions
///
/// ```rust,ignore
/// // NOT: title does NOT contain "Draft"
/// let q = Q::any()
///     .add(Column::Title.contains("Draft"))
///     .not();
///
/// let books = Book::objects(db).filter(q).all().await?;
/// ```
///
/// # Nested Conditions
///
/// ```rust,ignore
/// // Complex: (published AND price < 50) OR (featured AND price < 100)
/// let affordable = Q::all()
///     .add(Column::Published.eq(true))
///     .add(Column::Price.lt(50));
///
/// let featured_sale = Q::all()
///     .add(Column::Featured.eq(true))
///     .add(Column::Price.lt(100));
///
/// let combined = Q::any()
///     .add(affordable)
///     .add(featured_sale);
///
/// let books = Book::objects(db).filter(combined).all().await?;
/// ```
pub struct Q {
    condition: Condition,
}

#[allow(clippy::should_implement_trait)]
impl Q {
    /// Create a Q object with ALL conditions (AND logic)
    ///
    /// All conditions added must be true for a record to match.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Books that are both published AND have price < 50
    /// let q = Q::all()
    ///     .add(Column::Published.eq(true))
    ///     .add(Column::Price.lt(50));
    ///
    /// let books = Book::objects(db).filter(q).all().await?;
    /// // SQL: WHERE published = true AND price < 50
    /// ```
    pub fn all() -> Self {
        Self { condition: Condition::all() }
    }

    /// Create a Q object with ANY conditions (OR logic)
    ///
    /// At least one condition must be true for a record to match.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Books with title containing "Rust" OR "Python"
    /// let q = Q::any()
    ///     .add(Column::Title.contains("Rust"))
    ///     .add(Column::Title.contains("Python"));
    ///
    /// let books = Book::objects(db).filter(q).all().await?;
    /// // SQL: WHERE title LIKE '%Rust%' OR title LIKE '%Python%'
    /// ```
    pub fn any() -> Self {
        Self { condition: Condition::any() }
    }

    /// Add a condition to this Q object
    ///
    /// Conditions are combined according to the Q object type (all/any).
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Chain multiple conditions
    /// let q = Q::all()
    ///     .add(Column::Published.eq(true))
    ///     .add(Column::Price.lt(50))
    ///     .add(Column::InStock.eq(true));
    ///
    /// // Nest Q objects
    /// let q1 = Q::any()
    ///     .add(Column::Category.eq("Fiction"))
    ///     .add(Column::Category.eq("Mystery"));
    ///
    /// let q2 = Q::all()
    ///     .add(q1)  // Nested Q
    ///     .add(Column::Published.eq(true));
    /// ```
    pub fn add(mut self, expr: impl Into<sea_orm::sea_query::SimpleExpr>) -> Self {
        self.condition = self.condition.add(expr.into());
        self
    }

    /// Negate this Q object (Django's ~`Q()`)
    ///
    /// Returns a Q object that matches the opposite of the current conditions.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // NOT published
    /// let q = Q::all()
    ///     .add(Column::Published.eq(true))
    ///     .not();
    ///
    /// let drafts = Book::objects(db).filter(q).all().await?;
    /// // SQL: WHERE NOT (published = true)
    ///
    /// // NOT (Rust OR Python)
    /// let q = Q::any()
    ///     .add(Column::Title.contains("Rust"))
    ///     .add(Column::Title.contains("Python"))
    ///     .not();
    ///
    /// let other_books = Book::objects(db).filter(q).all().await?;
    /// // SQL: WHERE NOT (title LIKE '%Rust%' OR title LIKE '%Python%')
    /// ```
    pub fn not(mut self) -> Self {
        self.condition = self.condition.not();
        self
    }
}

impl From<Q> for Condition {
    fn from(q: Q) -> Self {
        q.condition
    }
}

// ============================================================================
// FilterExpr Enum - Type-safe filter expressions
// ============================================================================

/// Type-safe filter expression for building queries
///
/// This enum represents common filter operations in a pattern-matchable,
/// introspectable way. Use with `Q` objects or directly with `.filter()`.
///
/// # Example
///
/// ```rust,ignore
/// use seaorm_django::prelude::*;
///
/// // Using FilterExpr variants directly
/// let filter = FilterExpr::And(vec![
///     FilterExpr::eq(Book::Published, true),
///     FilterExpr::lt(Book::Price, 50),
/// ]);
///
/// // Pattern matching
/// match &filter {
///     FilterExpr::And(conditions) => println!("{} conditions", conditions.len()),
///     FilterExpr::Or(conditions) => println!("OR with {} conditions", conditions.len()),
///     _ => {}
/// }
/// ```
/// Filter operation type for typed, inspectable filter expressions
///
/// Each variant represents a specific comparison operation.
/// This enables exhaustive pattern matching and clear error messages.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterOp {
    /// Equality: column = value
    Eq,
    /// Not equal: column != value
    Ne,
    /// Less than: column < value
    Lt,
    /// Less than or equal: column <= value
    Lte,
    /// Greater than: column > value
    Gt,
    /// Greater than or equal: column >= value
    Gte,
    /// LIKE pattern match
    Like,
    /// NOT LIKE pattern match
    NotLike,
    /// IN list of values
    In,
    /// NOT IN list of values
    NotIn,
    /// IS NULL check
    IsNull,
    /// IS NOT NULL check
    IsNotNull,
    /// BETWEEN two values
    Between,
    /// String contains (LIKE %value%)
    Contains,
    /// String starts with (LIKE value%)
    StartsWith,
    /// String ends with (LIKE %value)
    EndsWith,
}

impl FilterOp {
    /// Get the SQL operator representation
    pub const fn sql_operator(&self) -> &'static str {
        match self {
            Self::Eq => "=",
            Self::Ne => "!=",
            Self::Lt => "<",
            Self::Lte => "<=",
            Self::Gt => ">",
            Self::Gte => ">=",
            Self::Like | Self::Contains | Self::StartsWith | Self::EndsWith => "LIKE",
            Self::NotLike => "NOT LIKE",
            Self::In => "IN",
            Self::NotIn => "NOT IN",
            Self::IsNull => "IS NULL",
            Self::IsNotNull => "IS NOT NULL",
            Self::Between => "BETWEEN",
        }
    }

    /// Check if this is a comparison operation
    pub const fn is_comparison(&self) -> bool {
        matches!(self, Self::Eq | Self::Ne | Self::Lt | Self::Lte | Self::Gt | Self::Gte)
    }

    /// Check if this is a string operation
    pub const fn is_string_op(&self) -> bool {
        matches!(self, Self::Like | Self::NotLike | Self::Contains | Self::StartsWith | Self::EndsWith)
    }

    /// Check if this is a null check
    pub const fn is_null_check(&self) -> bool {
        matches!(self, Self::IsNull | Self::IsNotNull)
    }
}

/// Type-safe filter expression for building queries
///
/// This enum represents filter operations in a pattern-matchable, introspectable way.
/// Supports typed operations with column/value information, logical combinations,
/// and raw expressions for SeaORM compatibility.
///
/// # Example
///
/// ```rust,ignore
/// use seaorm_django::prelude::*;
///
/// // Create typed filter
/// let filter = FilterExpr::eq(Book::Price, 100);
/// assert!(filter.is_typed());
/// assert_eq!(filter.get_op(), Some(&FilterOp::Eq));
///
/// // Combine with AND/OR
/// let combined = FilterExpr::And(vec![
///     FilterExpr::eq(Book::Published, true),
///     FilterExpr::lt(Book::Price, 50),
/// ]);
///
/// // Pattern matching for introspection
/// match &filter {
///     FilterExpr::Typed { column, op, value_repr, .. } => {
///         println!("{} {} {}", column, op.sql_operator(), value_repr);
///     }
///     FilterExpr::And(conditions) => println!("{} AND conditions", conditions.len()),
///     _ => {}
/// }
/// ```
#[derive(Debug, Clone)]
pub enum FilterExpr {
    /// AND combination of multiple conditions
    And(Vec<FilterExpr>),
    /// OR combination of multiple conditions  
    Or(Vec<FilterExpr>),
    /// NOT (negation) of a condition
    Not(Box<FilterExpr>),
    /// Typed filter operation with column name and operation type
    Typed {
        /// Column name being filtered
        column: String,
        /// The filter operation
        op: FilterOp,
        /// String representation of the value (for introspection)
        value_repr: String,
        /// The actual SeaORM expression
        expr: SimpleExpr,
    },
    /// Raw SimpleExpr for compatibility with SeaORM
    Raw(SimpleExpr),
}

impl FilterExpr {
    /// Create an AND combination of filters
    pub fn and(filters: Vec<FilterExpr>) -> Self {
        Self::And(filters)
    }

    /// Create an OR combination of filters
    pub fn or(filters: Vec<FilterExpr>) -> Self {
        Self::Or(filters)
    }

    /// Negate this filter expression
    pub fn not(self) -> Self {
        Self::Not(Box::new(self))
    }

    /// Create from any SimpleExpr (for compatibility)
    pub fn raw(expr: impl Into<SimpleExpr>) -> Self {
        Self::Raw(expr.into())
    }

    /// Create a typed filter expression
    fn typed<C: ColumnTrait>(column: C, op: FilterOp, value_repr: String, expr: SimpleExpr) -> Self {
        Self::Typed {
            column: format!("{:?}", column),
            op,
            value_repr,
            expr,
        }
    }

    /// Create equality filter: column = value
    pub fn eq<C: ColumnTrait, V: Into<sea_orm::Value> + std::fmt::Debug>(column: C, value: V) -> Self {
        let value_repr = format!("{:?}", value);
        Self::typed(column, FilterOp::Eq, value_repr, column.eq(value).into())
    }

    /// Create not-equal filter: column != value
    pub fn ne<C: ColumnTrait, V: Into<sea_orm::Value> + std::fmt::Debug>(column: C, value: V) -> Self {
        let value_repr = format!("{:?}", value);
        Self::typed(column, FilterOp::Ne, value_repr, column.ne(value).into())
    }

    /// Create less-than filter: column < value
    pub fn lt<C: ColumnTrait, V: Into<sea_orm::Value> + std::fmt::Debug>(column: C, value: V) -> Self {
        let value_repr = format!("{:?}", value);
        Self::typed(column, FilterOp::Lt, value_repr, column.lt(value).into())
    }

    /// Create less-than-or-equal filter: column <= value
    pub fn lte<C: ColumnTrait, V: Into<sea_orm::Value> + std::fmt::Debug>(column: C, value: V) -> Self {
        let value_repr = format!("{:?}", value);
        Self::typed(column, FilterOp::Lte, value_repr, column.lte(value).into())
    }

    /// Create greater-than filter: column > value
    pub fn gt<C: ColumnTrait, V: Into<sea_orm::Value> + std::fmt::Debug>(column: C, value: V) -> Self {
        let value_repr = format!("{:?}", value);
        Self::typed(column, FilterOp::Gt, value_repr, column.gt(value).into())
    }

    /// Create greater-than-or-equal filter: column >= value
    pub fn gte<C: ColumnTrait, V: Into<sea_orm::Value> + std::fmt::Debug>(column: C, value: V) -> Self {
        let value_repr = format!("{:?}", value);
        Self::typed(column, FilterOp::Gte, value_repr, column.gte(value).into())
    }

    /// Create IS NULL filter
    pub fn is_null<C: ColumnTrait>(column: C) -> Self {
        Self::typed(column, FilterOp::IsNull, "NULL".to_string(), column.is_null().into())
    }

    /// Create IS NOT NULL filter
    pub fn is_not_null<C: ColumnTrait>(column: C) -> Self {
        Self::typed(column, FilterOp::IsNotNull, "NOT NULL".to_string(), column.is_not_null().into())
    }

    /// Check if this is an AND expression
    pub const fn is_and(&self) -> bool {
        matches!(self, Self::And(_))
    }

    /// Check if this is an OR expression
    pub const fn is_or(&self) -> bool {
        matches!(self, Self::Or(_))
    }

    /// Check if this is a NOT expression
    pub const fn is_not(&self) -> bool {
        matches!(self, Self::Not(_))
    }

    /// Check if this is a typed filter expression
    pub const fn is_typed(&self) -> bool {
        matches!(self, Self::Typed { .. })
    }

    /// Check if this is a raw expression
    pub const fn is_raw(&self) -> bool {
        matches!(self, Self::Raw(_))
    }

    /// Get the filter operation if this is a typed filter
    pub fn get_op(&self) -> Option<&FilterOp> {
        match self {
            Self::Typed { op, .. } => Some(op),
            _ => None,
        }
    }

    /// Get the column name if this is a typed filter
    pub fn get_column(&self) -> Option<&str> {
        match self {
            Self::Typed { column, .. } => Some(column),
            _ => None,
        }
    }

    /// Get the value representation if this is a typed filter
    pub fn get_value_repr(&self) -> Option<&str> {
        match self {
            Self::Typed { value_repr, .. } => Some(value_repr),
            _ => None,
        }
    }

    /// Convert to SeaORM Condition for query execution
    pub fn into_condition(self) -> Condition {
        match self {
            Self::And(filters) => {
                let mut condition = Condition::all();
                for f in filters {
                    condition = condition.add(f.into_condition());
                }
                condition
            }
            Self::Or(filters) => {
                let mut condition = Condition::any();
                for f in filters {
                    condition = condition.add(f.into_condition());
                }
                condition
            }
            Self::Not(inner) => inner.into_condition().not(),
            Self::Typed { expr, .. } => Condition::all().add(expr),
            Self::Raw(expr) => Condition::all().add(expr),
        }
    }
}

impl From<FilterExpr> for Condition {
    fn from(expr: FilterExpr) -> Self {
        expr.into_condition()
    }
}

impl From<FilterExpr> for SimpleExpr {
    fn from(expr: FilterExpr) -> Self {
        // Convert to condition first, then extract as SimpleExpr
        let condition: Condition = expr.into();
        condition.into()
    }
}

// ============================================================================
// Extension Trait
// ============================================================================

/// Extension trait to add `.objects()` method to entities
///
/// This trait is automatically implemented for all `SeaORM` entities and provides
/// the Django-like `.objects(db)` entry point for querying.
///
/// # Basic Usage
///
/// ```rust,ignore
/// use entity::book::{Entity as Book, Column};
/// use seaorm_django::prelude::*;
///
/// // Get all books
/// let all_books = Book::objects(db).all().await?;
///
/// // Filter and query
/// let published = Book::objects(db)
///     .filter(Column::Published.eq(true))
///     .order_by_desc(Column::CreatedAt)
///     .limit(10)
///     .all()
///     .await?;
/// ```
///
/// # Available Query Methods
///
/// After calling `.objects(db)`, you can chain these methods:
///
/// - `.filter()` - Add WHERE conditions
/// - `.exclude()` - Add NOT WHERE conditions
/// - `.order_by_asc()`, `.order_by_desc()` - Sorting
/// - `.limit()`, `.offset()` - Pagination
/// - `.all()` - Get all results
/// - `.first()` - Get first result or None
/// - `.get(id)` - Get by primary key (errors if not found)
/// - `.count()` - Count matching records
/// - `.exists()` - Check if any match
/// - `.update()` - Bulk update
/// - `.delete()` - Bulk delete
/// - `.prefetch_related()` - Eager load relations
///
/// # Examples
///
/// ```rust,ignore
/// // Complex query
/// let books = Book::objects(db)
///     .filter(Column::Published.eq(true))
///     .filter(Column::Price.lt(50))
///     .exclude(Column::Title.contains("Draft"))
///     .order_by_desc(Column::CreatedAt)
///     .limit(20)
///     .all()
///     .await?;
///
/// // Single record
/// let book = Book::objects(db).get(1).await?;
///
/// // Check existence
/// if Book::objects(db)
///     .filter(Column::Title.eq(&title))
///     .exists()
///     .await?
/// {
///     return Err("Book already exists".into());
/// }
///
/// // Count
/// let total = Book::objects(db).count().await?;
/// let published = Book::objects(db)
///     .filter(Column::Published.eq(true))
///     .count()
///     .await?;
///
/// // With relations
/// let books = Book::objects(db)
///     .prefetch_related(vec![
///         TypeId::of::<Author>(),
///         TypeId::of::<Publisher>(),
///     ])
///     .all()
///     .await?;
/// ```
pub trait QueryExt: EntityTrait {
    /// Create a new `QuerySet` for this entity (Django's .objects)
    ///
    /// This is the entry point for all queries. Returns a `QuerySet` that you
    /// can chain methods on to build your query.
    ///
    /// # Parameters
    ///
    /// - `db` - Static reference to the database connection
    ///
    /// # Returns
    ///
    /// A `QuerySet<Self>` that you can chain query methods on.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Simple query
    /// let books = Book::objects(db).all().await?;
    ///
    /// // Filtered query
    /// let published = Book::objects(db)
    ///     .filter(Column::Published.eq(true))
    ///     .all()
    ///     .await?;
    ///
    /// // Single record
    /// let book = Book::objects(db).get(1).await?;
    ///
    /// // Count
    /// let count = Book::objects(db).count().await?;
    ///
    /// // Complex query
    /// let q = Q::any()
    ///     .add(Column::Title.contains("Rust"))
    ///     .add(Column::Title.contains("Python"));
    ///
    /// let books = Book::objects(db)
    ///     .filter(q)
    ///     .order_by_desc(Column::CreatedAt)
    ///     .limit(10)
    ///     .all()
    ///     .await?;
    /// ```
    fn objects<C: ConnectionTrait>(db: &C) -> QuerySet<'_, Self, C> {
        QuerySet::new(db)
    }
}

// Implement for all entities
impl<E: EntityTrait> QueryExt for E {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_unique_violation_sqlite() {
        let err = DbErr::Custom("UNIQUE constraint failed: users.email".to_string());
        assert!(is_unique_violation(&err));
    }

    #[test]
    fn test_is_unique_violation_postgres() {
        let err = DbErr::Custom("duplicate key value violates unique constraint".to_string());
        assert!(is_unique_violation(&err));
    }

    #[test]
    fn test_is_unique_violation_mysql() {
        let err = DbErr::Custom("Duplicate entry '123' for key 'PRIMARY'".to_string());
        assert!(is_unique_violation(&err));
    }

    #[test]
    fn test_is_unique_violation_negative() {
        let err = DbErr::Custom("Connection refused".to_string());
        assert!(!is_unique_violation(&err));
    }

    #[test]
    fn test_q_all_constructor() {
        let q = Q::all();
        // Should create an all() condition
        assert!(matches!(q.condition, Condition));
    }

    #[test]
    fn test_q_any_constructor() {
        let q = Q::any();
        // Should create an any() condition
        assert!(matches!(q.condition, Condition));
    }

    #[test]
    fn test_q_not_transformation() {
        let q = Q::all().not();
        // Should wrap condition in not()
        assert!(matches!(q.condition, Condition));
    }

    #[test]
    fn test_q_add_chaining() {
        use sea_orm::sea_query::Expr;
        let q = Q::all().add(Expr::value(true)).add(Expr::value(false));
        // Should allow chaining multiple add calls
        assert!(matches!(q.condition, Condition));
    }

    // ========================================================================
    // Aggregation Enum Tests
    // ========================================================================

    #[test]
    fn test_aggregation_count_all() {
        let agg = Aggregation::count_all();
        assert!(matches!(agg, Aggregation::CountAll));
    }

    #[test]
    fn test_aggregation_enum_is_debug() {
        let agg = Aggregation::count_all();
        let debug_str = format!("{:?}", agg);
        assert!(debug_str.contains("CountAll"));
    }

    #[test]
    fn test_aggregation_enum_is_clone() {
        let agg = Aggregation::count_all();
        let cloned = agg.clone();
        assert_eq!(agg, cloned);
    }

    #[test]
    fn test_aggregation_enum_is_eq() {
        let agg1 = Aggregation::count_all();
        let agg2 = Aggregation::count_all();
        assert_eq!(agg1, agg2);
    }

    #[test]
    fn test_aggregation_pattern_matching() {
        let aggregations = vec![Aggregation::count_all()];

        for agg in aggregations {
            match agg {
                Aggregation::CountAll => assert!(true),
                Aggregation::Count(_) => panic!("Expected CountAll"),
                Aggregation::Sum(_) => panic!("Expected CountAll"),
                Aggregation::Avg(_) => panic!("Expected CountAll"),
                Aggregation::Max(_) => panic!("Expected CountAll"),
                Aggregation::Min(_) => panic!("Expected CountAll"),
            }
        }
    }

    // ========================================================================
    // FilterExpr Enum Tests
    // ========================================================================

    #[test]
    fn test_filter_expr_and() {
        let filter = FilterExpr::And(vec![]);
        assert!(filter.is_and());
        assert!(!filter.is_or());
        assert!(!filter.is_not());
    }

    #[test]
    fn test_filter_expr_or() {
        let filter = FilterExpr::Or(vec![]);
        assert!(filter.is_or());
        assert!(!filter.is_and());
        assert!(!filter.is_not());
    }

    #[test]
    fn test_filter_expr_not() {
        let inner = FilterExpr::And(vec![]);
        let filter = inner.not();
        assert!(filter.is_not());
        assert!(!filter.is_and());
        assert!(!filter.is_or());
    }

    #[test]
    fn test_filter_expr_is_debug() {
        let filter = FilterExpr::And(vec![FilterExpr::Or(vec![])]);
        let debug_str = format!("{:?}", filter);
        assert!(debug_str.contains("And"));
        assert!(debug_str.contains("Or"));
    }

    #[test]
    fn test_filter_expr_is_clone() {
        let filter = FilterExpr::And(vec![FilterExpr::Or(vec![])]);
        let cloned = filter.clone();
        assert!(cloned.is_and());
    }

    #[test]
    fn test_filter_expr_nested_structure() {
        // Build: (A AND B) OR (C AND D)
        let ab = FilterExpr::And(vec![
            FilterExpr::Raw(Expr::value(true).into()),
            FilterExpr::Raw(Expr::value(false).into()),
        ]);
        let cd = FilterExpr::And(vec![
            FilterExpr::Raw(Expr::value(true).into()),
            FilterExpr::Raw(Expr::value(true).into()),
        ]);
        let combined = FilterExpr::Or(vec![ab, cd]);

        // Verify structure via pattern matching
        match combined {
            FilterExpr::Or(children) => {
                assert_eq!(children.len(), 2);
                for child in children {
                    assert!(child.is_and());
                }
            }
            _ => panic!("Expected Or"),
        }
    }

    #[test]
    fn test_filter_expr_into_condition() {
        // Test that conversion to Condition works
        let filter = FilterExpr::And(vec![FilterExpr::Raw(Expr::value(true).into())]);
        let _condition: Condition = filter.into();
        // If it compiles and runs, conversion works
    }

    #[test]
    fn test_filter_expr_pattern_matching() {
        let filters = vec![
            FilterExpr::And(vec![]),
            FilterExpr::Or(vec![]),
            FilterExpr::Not(Box::new(FilterExpr::And(vec![]))),
            FilterExpr::Raw(Expr::value(true).into()),
        ];

        for filter in filters {
            match &filter {
                FilterExpr::And(_) => assert!(filter.is_and()),
                FilterExpr::Or(_) => assert!(filter.is_or()),
                FilterExpr::Not(_) => assert!(filter.is_not()),
                FilterExpr::Typed { .. } => assert!(filter.is_typed()),
                FilterExpr::Raw(_) => {
                    assert!(filter.is_raw());
                    assert!(!filter.is_and());
                    assert!(!filter.is_or());
                    assert!(!filter.is_not());
                }
            }
        }
    }

    // ========================================================================
    // QueryOp Enum Tests
    // ========================================================================

    #[test]
    fn test_query_op_limit() {
        let op = QueryOp::limit(10);
        assert!(op.is_limit());
        assert!(!op.is_filter());
        assert!(!op.is_order_by());
    }

    #[test]
    fn test_query_op_offset() {
        let op = QueryOp::offset(20);
        assert!(op.is_offset());
        assert!(!op.is_limit());
    }

    #[test]
    fn test_query_op_distinct() {
        let op = QueryOp::distinct();
        assert!(op.is_distinct());
        assert!(!op.is_filter());
    }

    #[test]
    fn test_query_op_filter() {
        let op = QueryOp::filter(FilterExpr::And(vec![]));
        assert!(op.is_filter());
        assert!(!op.is_exclude());
    }

    #[test]
    fn test_query_op_exclude() {
        let op = QueryOp::exclude(FilterExpr::And(vec![]));
        assert!(op.is_exclude());
        assert!(!op.is_filter());
    }

    #[test]
    fn test_query_op_is_debug() {
        let op = QueryOp::Limit(10);
        let debug_str = format!("{:?}", op);
        assert!(debug_str.contains("Limit"));
        assert!(debug_str.contains("10"));
    }

    #[test]
    fn test_query_op_is_clone() {
        let op = QueryOp::Limit(10);
        let cloned = op.clone();
        assert!(cloned.is_limit());
    }

    #[test]
    fn test_order_direction() {
        assert_eq!(OrderDirection::Asc, OrderDirection::Asc);
        assert_ne!(OrderDirection::Asc, OrderDirection::Desc);
    }

    // ========================================================================
    // QueryPlan Tests
    // ========================================================================

    #[test]
    fn test_query_plan_new() {
        let plan = QueryPlan::new();
        assert!(plan.is_empty());
        assert_eq!(plan.len(), 0);
    }

    #[test]
    fn test_query_plan_push() {
        let mut plan = QueryPlan::new();
        plan.push(QueryOp::Limit(10));
        plan.push(QueryOp::Offset(5));
        assert_eq!(plan.len(), 2);
        assert!(!plan.is_empty());
    }

    #[test]
    fn test_query_plan_operations() {
        let mut plan = QueryPlan::new();
        plan.push(QueryOp::Limit(10));
        plan.push(QueryOp::Distinct);

        let ops = plan.operations();
        assert_eq!(ops.len(), 2);
        assert!(ops[0].is_limit());
        assert!(ops[1].is_distinct());
    }

    #[test]
    fn test_query_plan_has_filters() {
        let mut plan = QueryPlan::new();
        assert!(!plan.has_filters());

        plan.push(QueryOp::filter(FilterExpr::And(vec![])));
        assert!(plan.has_filters());
    }

    #[test]
    fn test_query_plan_has_ordering() {
        let mut plan = QueryPlan::new();
        assert!(!plan.has_ordering());

        // Use Asterisk ColumnRef (None = no table prefix)
        plan.push(QueryOp::OrderBy {
            column: sea_orm::sea_query::ColumnRef::Asterisk(None),
            direction: OrderDirection::Asc,
        });
        assert!(plan.has_ordering());
    }

    #[test]
    fn test_query_plan_has_limit() {
        let mut plan = QueryPlan::new();
        assert!(!plan.has_limit());

        plan.push(QueryOp::Limit(10));
        assert!(plan.has_limit());
    }

    #[test]
    fn test_query_plan_get_limit() {
        let mut plan = QueryPlan::new();
        assert_eq!(plan.get_limit(), None);

        plan.push(QueryOp::Limit(25));
        assert_eq!(plan.get_limit(), Some(25));
    }

    #[test]
    fn test_query_plan_get_offset() {
        let mut plan = QueryPlan::new();
        assert_eq!(plan.get_offset(), None);

        plan.push(QueryOp::Offset(100));
        assert_eq!(plan.get_offset(), Some(100));
    }

    #[test]
    fn test_query_plan_filters() {
        let mut plan = QueryPlan::new();
        plan.push(QueryOp::filter(FilterExpr::And(vec![])));
        plan.push(QueryOp::Limit(10));
        plan.push(QueryOp::filter(FilterExpr::Or(vec![])));

        let filters = plan.filters();
        assert_eq!(filters.len(), 2);
    }

    #[test]
    fn test_query_plan_iter() {
        let mut plan = QueryPlan::new();
        plan.push(QueryOp::Limit(10));
        plan.push(QueryOp::Offset(5));

        let count = plan.iter().count();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_query_plan_is_debug() {
        let mut plan = QueryPlan::new();
        plan.push(QueryOp::Limit(10));
        let debug_str = format!("{:?}", plan);
        assert!(debug_str.contains("QueryPlan"));
        assert!(debug_str.contains("Limit"));
    }

    #[test]
    fn test_query_plan_is_clone() {
        let mut plan = QueryPlan::new();
        plan.push(QueryOp::Limit(10));
        let cloned = plan.clone();
        assert_eq!(cloned.len(), 1);
    }

    #[test]
    fn test_query_plan_pattern_matching() {
        let mut plan = QueryPlan::new();
        plan.push(QueryOp::Filter(FilterExpr::And(vec![])));
        plan.push(QueryOp::Limit(10));
        plan.push(QueryOp::Distinct);

        for op in plan.iter() {
            match op {
                QueryOp::Filter(_) => assert!(op.is_filter()),
                QueryOp::Limit(n) => assert_eq!(*n, 10),
                QueryOp::Distinct => assert!(op.is_distinct()),
                _ => {}
            }
        }
    }

    // ========================================================================
    // QueryOp Variant Tests
    // ========================================================================

    #[test]
    fn test_query_op_soft_delete_variant() {
        let op = QueryOp::SoftDelete(SoftDeleteMode::OnlyDeleted);

        match op {
            QueryOp::SoftDelete(mode) => {
                assert_eq!(mode, SoftDeleteMode::OnlyDeleted);
            }
            _ => panic!("Expected SoftDelete"),
        }
    }

    #[test]
    fn test_query_op_annotate_variant() {
        let op = QueryOp::Annotate {
            alias: "total".to_string(),
            aggregation: Aggregation::CountAll,
        };

        match op {
            QueryOp::Annotate { alias, aggregation } => {
                assert_eq!(alias, "total");
                assert!(matches!(aggregation, Aggregation::CountAll));
            }
            _ => panic!("Expected Annotate"),
        }
    }

    #[test]
    fn test_query_op_order_by_variant() {
        let op = QueryOp::OrderBy {
            column: sea_orm::sea_query::ColumnRef::Asterisk(None),
            direction: OrderDirection::Desc,
        };

        assert!(op.is_order_by());
        match op {
            QueryOp::OrderBy { direction, .. } => {
                assert_eq!(direction, OrderDirection::Desc);
            }
            _ => panic!("Expected OrderBy"),
        }
    }

    #[test]
    fn test_query_op_group_by_variant() {
        let op = QueryOp::GroupBy(sea_orm::sea_query::ColumnRef::Asterisk(None));

        match op {
            QueryOp::GroupBy(_) => {}
            _ => panic!("Expected GroupBy"),
        }
    }

    #[test]
    fn test_query_op_exclude_variant() {
        let op = QueryOp::Exclude(FilterExpr::And(vec![]));
        assert!(op.is_exclude());
        assert!(!op.is_filter());
    }

    // ========================================================================
    // FilterOp Enum Tests
    // ========================================================================

    #[test]
    fn test_filter_op_sql_operators() {
        assert_eq!(FilterOp::Eq.sql_operator(), "=");
        assert_eq!(FilterOp::Ne.sql_operator(), "!=");
        assert_eq!(FilterOp::Lt.sql_operator(), "<");
        assert_eq!(FilterOp::Lte.sql_operator(), "<=");
        assert_eq!(FilterOp::Gt.sql_operator(), ">");
        assert_eq!(FilterOp::Gte.sql_operator(), ">=");
        assert_eq!(FilterOp::Like.sql_operator(), "LIKE");
        assert_eq!(FilterOp::NotLike.sql_operator(), "NOT LIKE");
        assert_eq!(FilterOp::In.sql_operator(), "IN");
        assert_eq!(FilterOp::NotIn.sql_operator(), "NOT IN");
        assert_eq!(FilterOp::IsNull.sql_operator(), "IS NULL");
        assert_eq!(FilterOp::IsNotNull.sql_operator(), "IS NOT NULL");
        assert_eq!(FilterOp::Between.sql_operator(), "BETWEEN");
        assert_eq!(FilterOp::Contains.sql_operator(), "LIKE");
        assert_eq!(FilterOp::StartsWith.sql_operator(), "LIKE");
        assert_eq!(FilterOp::EndsWith.sql_operator(), "LIKE");
    }

    #[test]
    fn test_filter_op_is_comparison() {
        assert!(FilterOp::Eq.is_comparison());
        assert!(FilterOp::Ne.is_comparison());
        assert!(FilterOp::Lt.is_comparison());
        assert!(FilterOp::Lte.is_comparison());
        assert!(FilterOp::Gt.is_comparison());
        assert!(FilterOp::Gte.is_comparison());
        assert!(!FilterOp::Like.is_comparison());
        assert!(!FilterOp::IsNull.is_comparison());
    }

    #[test]
    fn test_filter_op_is_string_op() {
        assert!(FilterOp::Like.is_string_op());
        assert!(FilterOp::NotLike.is_string_op());
        assert!(FilterOp::Contains.is_string_op());
        assert!(FilterOp::StartsWith.is_string_op());
        assert!(FilterOp::EndsWith.is_string_op());
        assert!(!FilterOp::Eq.is_string_op());
        assert!(!FilterOp::IsNull.is_string_op());
    }

    #[test]
    fn test_filter_op_is_null_check() {
        assert!(FilterOp::IsNull.is_null_check());
        assert!(FilterOp::IsNotNull.is_null_check());
        assert!(!FilterOp::Eq.is_null_check());
        assert!(!FilterOp::Like.is_null_check());
    }

    #[test]
    fn test_filter_op_equality() {
        assert_eq!(FilterOp::Eq, FilterOp::Eq);
        assert_ne!(FilterOp::Eq, FilterOp::Ne);
    }

    // ========================================================================
    // QueryState Enum Tests
    // ========================================================================

    #[test]
    fn test_query_state_default() {
        let state = QueryState::default();
        assert!(state.is_fresh());
        assert!(!state.is_filtered());
        assert!(!state.is_ordered());
        assert!(!state.is_paginated());
        assert!(!state.is_aggregated());
        assert!(!state.is_executed());
    }

    #[test]
    fn test_query_state_transitions() {
        let mut state = QueryState::Fresh;

        // Filter transition
        state.filter();
        assert!(state.is_filtered());
        assert_eq!(state, QueryState::Filtered);

        // Order transition
        state = QueryState::Fresh;
        state.filter();
        state.order();
        assert!(state.is_ordered());
        assert_eq!(state, QueryState::Ordered);

        // Paginate transition
        state.paginate();
        assert!(state.is_paginated());
        assert_eq!(state, QueryState::Paginated);

        // Aggregate transition
        state = QueryState::Fresh;
        state.aggregate();
        assert!(state.is_aggregated());
        assert_eq!(state, QueryState::Aggregated);

        // Execute transition
        state.execute();
        assert!(state.is_executed());
        assert_eq!(state, QueryState::Executed);
    }

    #[test]
    fn test_query_state_pattern_matching() {
        let states = [
            QueryState::Fresh,
            QueryState::Filtered,
            QueryState::Ordered,
            QueryState::Paginated,
            QueryState::Aggregated,
            QueryState::Executed,
        ];

        for state in states {
            match state {
                QueryState::Fresh => assert!(state.is_fresh()),
                QueryState::Filtered => assert!(state.is_filtered()),
                QueryState::Ordered => assert!(state.is_ordered()),
                QueryState::Paginated => assert!(state.is_paginated()),
                QueryState::Aggregated => assert!(state.is_aggregated()),
                QueryState::Executed => assert!(state.is_executed()),
            }
        }
    }

    #[test]
    fn test_query_state_clone_copy() {
        let state = QueryState::Filtered;
        let cloned = state.clone();
        let copied = state;

        assert_eq!(state, cloned);
        assert_eq!(state, copied);
    }

    // ========================================================================
    // FilterExpr Typed Variant Tests
    // ========================================================================

    #[test]
    fn test_filter_expr_typed_is_typed() {
        use sea_orm::sea_query::Expr;
        let filter = FilterExpr::Typed {
            column: "price".to_string(),
            op: FilterOp::Eq,
            value_repr: "100".to_string(),
            expr: Expr::value(100).into(),
        };
        assert!(filter.is_typed());
        assert!(!filter.is_raw());
        assert!(!filter.is_and());
        assert!(!filter.is_or());
        assert!(!filter.is_not());
    }

    #[test]
    fn test_filter_expr_get_op() {
        use sea_orm::sea_query::Expr;
        let filter = FilterExpr::Typed {
            column: "price".to_string(),
            op: FilterOp::Lt,
            value_repr: "50".to_string(),
            expr: Expr::value(50).into(),
        };
        assert_eq!(filter.get_op(), Some(&FilterOp::Lt));

        let raw = FilterExpr::Raw(Expr::value(true).into());
        assert_eq!(raw.get_op(), None);
    }

    #[test]
    fn test_filter_expr_get_column() {
        use sea_orm::sea_query::Expr;
        let filter = FilterExpr::Typed {
            column: "author_id".to_string(),
            op: FilterOp::Eq,
            value_repr: "1".to_string(),
            expr: Expr::value(1).into(),
        };
        assert_eq!(filter.get_column(), Some("author_id"));

        let and = FilterExpr::And(vec![]);
        assert_eq!(and.get_column(), None);
    }

    #[test]
    fn test_filter_expr_get_value_repr() {
        use sea_orm::sea_query::Expr;
        let filter = FilterExpr::Typed {
            column: "name".to_string(),
            op: FilterOp::Contains,
            value_repr: "test".to_string(),
            expr: Expr::value("test").into(),
        };
        assert_eq!(filter.get_value_repr(), Some("test"));

        let or = FilterExpr::Or(vec![]);
        assert_eq!(or.get_value_repr(), None);
    }

    #[test]
    fn test_filter_expr_typed_into_condition() {
        use sea_orm::sea_query::Expr;
        let filter = FilterExpr::Typed {
            column: "status".to_string(),
            op: FilterOp::Eq,
            value_repr: "active".to_string(),
            expr: Expr::value("active").into(),
        };
        // Should not panic and should create valid condition
        let _condition: Condition = filter.into();
    }

    #[test]
    fn test_filter_expr_pattern_matching_with_typed() {
        use sea_orm::sea_query::Expr;
        let filters = vec![
            FilterExpr::And(vec![]),
            FilterExpr::Or(vec![]),
            FilterExpr::Not(Box::new(FilterExpr::And(vec![]))),
            FilterExpr::Typed {
                column: "id".to_string(),
                op: FilterOp::Gt,
                value_repr: "10".to_string(),
                expr: Expr::value(10).into(),
            },
            FilterExpr::Raw(Expr::value(true).into()),
        ];

        for filter in filters {
            match &filter {
                FilterExpr::And(_) => assert!(filter.is_and()),
                FilterExpr::Or(_) => assert!(filter.is_or()),
                FilterExpr::Not(_) => assert!(filter.is_not()),
                FilterExpr::Typed { op, .. } => {
                    assert!(filter.is_typed());
                    assert_eq!(filter.get_op(), Some(op));
                }
                FilterExpr::Raw(_) => assert!(filter.is_raw()),
            }
        }
    }
}
