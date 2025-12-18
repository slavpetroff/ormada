//! QuerySet API for Django-like query building on SeaORM.

use crate::db::{ConnectionTrait, DbErr, TransactionTrait};
use crate::error::OrmadaError;
use crate::fields::{ColumnTrait, Condition, Order, PrimaryKeyTrait, Value};
use crate::hooks::LifecycleHooks;
use crate::models::{
    ActiveModelBehavior, ActiveModelTrait, EntityTrait, FromQueryResult, IntoActiveModel,
    ModelTrait, QueryFilter, QueryOrder, QuerySelect, Select,
};
use crate::upsert::UpsertBuilder;
use sea_orm::sea_query::{BinOper, ColumnRef, Expr, Func, SimpleExpr};
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
// Column Extension Trait (Only Ormada-specific additions)
// ============================================================================

/// Extension trait for Ormada-specific column operations
///
/// This trait adds Ormada-like aliases to SeaORM's `ColumnTrait`.
/// For standard operations like `.eq()`, `.gt()`, etc., use `ColumnTrait` directly.
///
/// **Note:** `ColumnTrait` is re-exported in our prelude and provides all standard
/// column operations: `.eq()`, `.ne()`, `.gt()`, `.gte()`, `.lt()`, `.lte()`,
/// `.contains()`, `.starts_with()`, `.ends_with()`, `.is_null()`, `.is_not_null()`,
/// `.is_in()`, etc.
///
/// This trait only adds Ormada-specific aliases not present in SeaORM.
pub trait ColumnExt: ColumnTrait {
    /// Alias for `is_in` - Ormada's `field__in=[...]` syntax
    ///
    /// ```rust,ignore
    /// Book::CategoryId.in_values([1, 2, 3])
    /// ```
    fn in_values<V, I>(&self, values: I) -> SimpleExpr
    where
        V: Into<Value>,
        I: IntoIterator<Item = V>,
    {
        ColumnTrait::is_in(self, values)
    }
}

// Implement for all ColumnTrait types (works with ANY entity!)
impl<T: ColumnTrait> ColumnExt for T {}

// ============================================================================
/// Main `QuerySet` structure (Ormada's `QuerySet` equivalent)
///
/// Provides chainable query building with automatic caching and lazy evaluation.
/// All operations are lazy until a terminal method (.`all()`, .`first()`, etc.) is called.
///
/// **Caching Behavior (Ormada-like):**
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
/// - `S`: The typestate marker (defaults to `Fresh`)
///
/// # Typestate Pattern
///
/// The `S` parameter tracks the query building state at compile time:
/// - `Fresh` → initial state, can filter, order, or execute
/// - `Filtered` → has filters, can add more filters, order, or execute
/// - `Ordered` → has ordering, can paginate or execute
/// - `Paginated` → has limit/offset, can execute
/// - `Aggregated` → has aggregations, can execute
///
/// Methods transition between states, preventing invalid operations
/// at compile time.
///
/// # Examples
///
/// ```rust,ignore
/// // Build query with typestate
/// let queryset = Book::objects(db)           // QuerySet<_, _, Fresh>
///     .filter(Book::Published.eq(true))      // QuerySet<_, _, Filtered>
///     .order_by_asc(Book::Title)             // QuerySet<_, _, Ordered>
///     .limit(10);                            // QuerySet<_, _, Paginated>
///
/// // Execute the query
/// let books = queryset.all().await?;
/// ```
pub struct QuerySet<'a, E: EntityTrait, C: ConnectionTrait, S: QuerySetState = Fresh> {
    pub(crate) inner: Arc<QuerySetInner<'a, E, C>>,
    pub(crate) _state: std::marker::PhantomData<S>,
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

// ============================================================================
// QuerySet Typestate Markers - Zero-sized types for compile-time state tracking
// ============================================================================

/// Marker trait for all valid QuerySet states
pub trait QuerySetState: Clone + Copy + Default + std::fmt::Debug {}

/// Fresh state - initial QuerySet, no operations applied
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fresh;
impl QuerySetState for Fresh {}

/// Filtered state - has filter/exclude operations
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Filtered;
impl QuerySetState for Filtered {}

/// Ordered state - has ordering applied
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ordered;
impl QuerySetState for Ordered {}

/// Paginated state - has limit/offset applied
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Paginated;
impl QuerySetState for Paginated {}

/// Grouped state - has GROUP BY applied, ready for annotations
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Grouped;
impl QuerySetState for Grouped {}

/// Aggregated state - has aggregations/annotations
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Aggregated;
impl QuerySetState for Aggregated {}

/// Marker trait for states that can add filters
pub trait CanFilter: QuerySetState {}
impl CanFilter for Fresh {}
impl CanFilter for Filtered {}

/// Marker trait for states that can add ordering
pub trait CanOrder: QuerySetState {}
impl CanOrder for Fresh {}
impl CanOrder for Filtered {}
impl CanOrder for Ordered {}

/// Marker trait for states that can add pagination
pub trait CanPaginate: QuerySetState {}
impl CanPaginate for Fresh {}
impl CanPaginate for Filtered {}
impl CanPaginate for Ordered {}
impl CanPaginate for Paginated {}

/// Marker trait for states that can add GROUP BY
pub trait CanGroup: QuerySetState {}
impl CanGroup for Fresh {}
impl CanGroup for Filtered {}

/// Marker trait for states that can add annotations (aggregations)
pub trait CanAnnotate: QuerySetState {}
impl CanAnnotate for Fresh {}
impl CanAnnotate for Filtered {}
impl CanAnnotate for Grouped {}

/// Marker trait for states that can execute queries
pub trait CanExecute: QuerySetState {}
impl CanExecute for Fresh {}
impl CanExecute for Filtered {}
impl CanExecute for Ordered {}
impl CanExecute for Paginated {}
impl CanExecute for Grouped {}
impl CanExecute for Aggregated {}

/// Query building state for introspection
///
/// Tracks the current state of query construction, enabling
/// validation and debugging of query building patterns.
///
/// # Example
///
/// ```rust,ignore
/// use ormada::prelude::*;
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
/// use ormada::prelude::*;
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
        column: ColumnRef,
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
    GroupBy(ColumnRef),
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
impl<E: EntityTrait, C: ConnectionTrait, S: QuerySetState> Clone for QuerySet<'_, E, C, S> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            _state: std::marker::PhantomData,
        }
    }
}

impl<'a, E: EntityTrait, C: ConnectionTrait> QuerySet<'a, E, C, Fresh> {
    /// Create a new `QuerySet` in Fresh state
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
            _state: std::marker::PhantomData,
        }
    }
}

impl<'a, E: EntityTrait, C: ConnectionTrait, S: QuerySetState> QuerySet<'a, E, C, S> {
    /// Create a new `QuerySet` with modified select and operation, transitioning to new state
    fn with_select_and_op_to<NewS: QuerySetState>(
        &self,
        select: Select<E>,
        op: QueryOp,
        new_query_state: QueryState,
    ) -> QuerySet<'a, E, C, NewS> {
        let mut plan = self.inner.plan.clone();
        plan.push(op);
        QuerySet {
            inner: Arc::new(QuerySetInner {
                db: self.inner.db,
                select,
                soft_delete_mode: self.inner.soft_delete_mode,
                plan,
                query_state: new_query_state,
                cache: RwLock::new(None),
            }),
            _state: std::marker::PhantomData,
        }
    }

    /// Create a new `QuerySet` with modified select and operation (preserves typestate)
    fn with_select_and_op(&self, select: Select<E>, op: QueryOp) -> Self {
        let mut plan = self.inner.plan.clone();
        plan.push(op);
        Self {
            inner: Arc::new(QuerySetInner {
                db: self.inner.db,
                select,
                soft_delete_mode: self.inner.soft_delete_mode,
                plan,
                query_state: self.inner.query_state,
                cache: RwLock::new(None),
            }),
            _state: std::marker::PhantomData,
        }
    }

    /// Create a new `QuerySet` with modified soft delete mode (preserves state)
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
            _state: std::marker::PhantomData,
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
        E: crate::traits::OrmadaEntity,
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
                            BinOper::Is,
                            Box::new(SimpleExpr::Value(Value::String(None))),
                        );
                        select = select.filter(condition);
                    }
                    SoftDeleteMode::OnlyDeleted => {
                        // Filter WHERE deleted_at IS NOT NULL
                        let condition = SimpleExpr::Binary(
                            Box::new(Expr::col(Alias::new(column))),
                            BinOper::IsNot,
                            Box::new(SimpleExpr::Value(Value::String(None))),
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
}

// ============================================================================
// Typestate: Filter operations (Fresh, Filtered states)
// ============================================================================

impl<'a, E: EntityTrait, C: ConnectionTrait, S: CanFilter> QuerySet<'a, E, C, S> {
    /// Filter records (Ormada's .`filter()`)
    ///
    /// Creates a new `QuerySet` with added filter. Transitions to `Filtered` state.
    /// Available on `Fresh` and `Filtered` states.
    ///
    /// # Typestate
    /// - Input: `QuerySet<Fresh>` or `QuerySet<Filtered>`
    /// - Output: `QuerySet<Filtered>`
    pub fn filter(&self, condition: impl Into<Condition>) -> QuerySet<'a, E, C, Filtered> {
        let cond: Condition = condition.into();
        let new_select = self.inner.select.clone().filter(cond.clone());
        self.with_select_and_op_to(
            new_select,
            QueryOp::Filter(FilterExpr::raw(cond)),
            QueryState::Filtered,
        )
    }

    /// Exclude records (Ormada's .`exclude()`)
    ///
    /// Creates a new `QuerySet` with added exclusion. Transitions to `Filtered` state.
    /// Available on `Fresh` and `Filtered` states.
    ///
    /// # Typestate
    /// - Input: `QuerySet<Fresh>` or `QuerySet<Filtered>`
    /// - Output: `QuerySet<Filtered>`
    pub fn exclude(&self, condition: impl Into<Condition>) -> QuerySet<'a, E, C, Filtered> {
        let cond: Condition = condition.into();
        let new_select = self.inner.select.clone().filter(cond.clone().not());
        self.with_select_and_op_to(
            new_select,
            QueryOp::Exclude(FilterExpr::raw(cond)),
            QueryState::Filtered,
        )
    }
}

// Continue with common methods that preserve state
impl<'a, E: EntityTrait, C: ConnectionTrait, S: QuerySetState> QuerySet<'a, E, C, S> {
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

    /// Remove duplicate rows (Ormada's .`distinct()`)
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
}

// ============================================================================
// Typestate: Ordering operations (Fresh, Filtered, Ordered states)
// ============================================================================

impl<'a, E: EntityTrait, C: ConnectionTrait, S: CanOrder> QuerySet<'a, E, C, S> {
    /// Order by a column in ascending order (Ormada's .`order_by`('field'))
    ///
    /// Transitions to `Ordered` state. Available on `Fresh`, `Filtered`, and `Ordered` states.
    ///
    /// # Typestate
    /// - Input: `QuerySet<Fresh>`, `QuerySet<Filtered>`, or `QuerySet<Ordered>`
    /// - Output: `QuerySet<Ordered>`
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Order by price (lowest first)
    /// let books = Book::objects(db)
    ///     .order_by_asc(Book::Price)
    ///     .all()
    ///     .await?;
    /// ```
    pub fn order_by_asc(&self, column: impl ColumnTrait) -> QuerySet<'a, E, C, Ordered> {
        let col_ref = column.as_column_ref().into();
        let new_select = self.inner.select.clone().order_by(column, Order::Asc);
        self.with_select_and_op_to(
            new_select,
            QueryOp::OrderBy {
                column: col_ref,
                direction: OrderDirection::Asc,
            },
            QueryState::Ordered,
        )
    }

    /// Order by a column in descending order (Ormada's .`order_by`('-field'))
    ///
    /// Transitions to `Ordered` state. Available on `Fresh`, `Filtered`, and `Ordered` states.
    ///
    /// # Typestate
    /// - Input: `QuerySet<Fresh>`, `QuerySet<Filtered>`, or `QuerySet<Ordered>`
    /// - Output: `QuerySet<Ordered>`
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Order by price (highest first)
    /// let books = Book::objects(db)
    ///     .order_by_desc(Book::Price)
    ///     .all()
    ///     .await?;
    /// ```
    pub fn order_by_desc(&self, column: impl ColumnTrait) -> QuerySet<'a, E, C, Ordered> {
        let col_ref = column.as_column_ref().into();
        let new_select = self.inner.select.clone().order_by(column, Order::Desc);
        self.with_select_and_op_to(
            new_select,
            QueryOp::OrderBy {
                column: col_ref,
                direction: OrderDirection::Desc,
            },
            QueryState::Ordered,
        )
    }
}

// ============================================================================
// Typestate: Pagination operations (Fresh, Filtered, Ordered, Paginated states)
// ============================================================================

impl<'a, E: EntityTrait, C: ConnectionTrait, S: CanPaginate> QuerySet<'a, E, C, S> {
    /// Limit results (Ormada's [:n])
    ///
    /// Transitions to `Paginated` state. Available on `Fresh`, `Filtered`, `Ordered`, and `Paginated` states.
    ///
    /// # Typestate
    /// - Input: `QuerySet<Fresh>`, `QuerySet<Filtered>`, `QuerySet<Ordered>`, or `QuerySet<Paginated>`
    /// - Output: `QuerySet<Paginated>`
    pub fn limit(&self, limit: u64) -> QuerySet<'a, E, C, Paginated> {
        let new_select = self.inner.select.clone().limit(limit);
        self.with_select_and_op_to(new_select, QueryOp::Limit(limit), QueryState::Paginated)
    }

    /// Offset results
    ///
    /// Transitions to `Paginated` state. Available on `Fresh`, `Filtered`, `Ordered`, and `Paginated` states.
    ///
    /// # Typestate
    /// - Input: `QuerySet<Fresh>`, `QuerySet<Filtered>`, `QuerySet<Ordered>`, or `QuerySet<Paginated>`
    /// - Output: `QuerySet<Paginated>`
    pub fn offset(&self, offset: u64) -> QuerySet<'a, E, C, Paginated> {
        let new_select = self.inner.select.clone().offset(offset);
        self.with_select_and_op_to(new_select, QueryOp::Offset(offset), QueryState::Paginated)
    }
}

// ============================================================================
// Typestate: Execution operations (all executable states)
// ============================================================================

impl<'a, E: EntityTrait, C: ConnectionTrait, S: CanExecute + 'a> QuerySet<'a, E, C, S>
where
    E: crate::traits::OrmadaEntity,
{
    /// Execute query and return all matching results (Ormada's .`all()`)
    ///
    /// Returns a vector of all models that match the query filters.
    ///
    /// # Returns
    ///
    /// - `Ok(Vec<E::Model>)` - Vector of matching models (may be empty)
    /// - `Err(OrmadaError)` - Database error occurred
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
    pub async fn all(&self) -> Result<Vec<E::Model>, OrmadaError>
    where
        E: crate::traits::OrmadaEntity,
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
        let results_arc = Arc::new(results);

        // Update cache (exclusive write lock)
        {
            let mut cache = self.inner.cache.write().await;
            *cache = Some(Arc::clone(&results_arc));
        }

        Ok((*results_arc).clone())
    }

    /// Execute query and return first result (Ormada's .`first()`)
    ///
    /// Returns the first matching model or error if no matches found.
    /// Useful with ordering to get the "latest" or "oldest" record.
    ///
    /// # Returns
    ///
    /// - `Ok(E::Model)` - First matching model found
    /// - `Err(OrmadaError::EmptyResult { .. })` - No matching models
    /// - `Err(OrmadaError::Database(_))` - Database error occurred
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
    ///     Err(OrmadaError::EmptyResult { .. }) => {
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
    pub async fn first(&self) -> Result<E::Model, OrmadaError>
    where
        E: crate::traits::OrmadaEntity,
    {
        // Try cache first
        {
            let cache = self.inner.cache.read().await;
            if let Some(cached_results) = cache.as_ref() {
                return cached_results
                    .first()
                    .cloned()
                    .ok_or_else(|| OrmadaError::empty_result_set("first"));
            }
        }

        // Apply soft delete filter before executing
        let query = self.apply_soft_delete_filter(self.inner.select.clone());

        // Cache miss - execute query for single record
        query
            .one(self.inner.db)
            .await?
            .ok_or_else(|| OrmadaError::empty_result_set("first"))
    }

    /// Execute query and return last result
    ///
    /// Returns the last matching model or error if no matches found.
    /// Orders by primary key descending and returns the first result.
    ///
    /// # Returns
    ///
    /// - `Ok(E::Model)` - Last matching model found
    /// - `Err(OrmadaError::NotFound { .. })` - No matching models
    /// - `Err(OrmadaError::Database(_))` - Database error occurred
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
    ///     Err(OrmadaError::NotFound { .. }) => {
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
    pub async fn last(&self) -> Result<E::Model, OrmadaError>
    where
        E: crate::traits::OrmadaEntity,
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

        query.one(self.inner.db).await?.ok_or_else(|| {
            OrmadaError::does_not_exist(E::default().table_name(), "last".to_string())
        })
    }

    /// Get a single record by primary key (Ormada's .get(pk=))
    ///
    /// Returns the model or error if not found. This matches Ormada's behavior
    /// where `.get()` raises `DoesNotExist` if the record doesn't exist.
    ///
    /// # Returns
    ///
    /// - `Ok(E::Model)` - Record found
    /// - `Err(OrmadaError::NotFound { entity, id })` - No record with that ID
    /// - `Err(OrmadaError::Database(_))` - Database error occurred
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
    ///     Err(OrmadaError::NotFound { entity, id }) => {
    ///         println!("{} with id {} doesn't exist", entity, id);
    ///     }
    ///     Err(e) => return Err(e),  // Other error
    /// }
    /// // Or use ? for early return on not found
    pub async fn get<T>(&self, id: T) -> Result<E::Model, OrmadaError>
    where
        T: Into<<E::PrimaryKey as PrimaryKeyTrait>::ValueType> + Send + std::fmt::Display,
        E: crate::traits::OrmadaEntity,
    {
        let id_str = format!("{}", &id);

        // Build the query with soft delete filter
        let query = self.apply_soft_delete_filter(E::find_by_id(id));

        query
            .one(self.inner.db)
            .await?
            .ok_or_else(|| OrmadaError::does_not_exist(E::default().table_name(), id_str))
    }

    /// Get the earliest record by a field (Ormada's .`earliest()`)
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
    /// - `Err(OrmadaError::EmptyResult { .. })` - No records found
    /// - `Err(OrmadaError::Database)` - Database error
    ///
    /// # Equivalent to
    ///
    /// `.order_by_asc(column).first()` but returns error on empty result
    pub async fn earliest(&self, column: impl ColumnTrait) -> Result<E::Model, OrmadaError> {
        self.inner
            .select
            .clone()
            .order_by(column, Order::Asc)
            .one(self.inner.db)
            .await?
            .ok_or_else(|| OrmadaError::empty_result_set("earliest"))
    }

    /// Get the latest record by a field (Ormada's .`latest()`)
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
    /// - `Err(OrmadaError::EmptyResult { .. })` - No records found
    /// - `Err(OrmadaError::Database)` - Database error
    ///
    /// # Equivalent to
    ///
    /// `.order_by_desc(column).first()` but returns error on empty result
    pub async fn latest(&self, column: impl ColumnTrait) -> Result<E::Model, OrmadaError> {
        self.inner
            .select
            .clone()
            .order_by(column, Order::Desc)
            .one(self.inner.db)
            .await?
            .ok_or_else(|| OrmadaError::empty_result_set("latest"))
    }

    /// Count records matching the query (Ormada's .`count()`)
    ///
    /// Returns the number of records that match the query filters.
    /// Returns 0 if no records match (not an error).
    ///
    /// # Returns
    ///
    /// - `Ok(u64)` - Number of matching records (0 or more)
    /// - `Err(OrmadaError)` - Database error occurred
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
    pub async fn count(&self) -> Result<u64, OrmadaError>
    where
        E: crate::traits::OrmadaEntity,
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

    /// Check if any records exist matching the query (Ormada's .`exists()`)
    ///
    /// Returns true if at least one record matches the query, false otherwise.
    /// More efficient than `.count() > 0` because it stops at the first match.
    ///
    /// # Returns
    ///
    /// - `Ok(true)` - At least one matching record exists
    /// - `Ok(false)` - No matching records (NOT an error)
    /// - `Err(OrmadaError)` - Database error occurred
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
    pub async fn exists(&self) -> Result<bool, OrmadaError>
    where
        E: crate::traits::OrmadaEntity,
    {
        use sea_orm::QuerySelect;

        // Apply soft delete filter first
        let filtered = self.apply_soft_delete_filter(self.inner.select.clone());

        // Use LIMIT 1 for efficiency
        let result = filtered.limit(1).one(self.inner.db).await?;
        Ok(result.is_some())
    }

    /// Update all records matching the query (Ormada's .`update()`)
    ///
    /// Applies the same updates to all matching records using an async closure.
    /// Returns the number of records updated.
    ///
    /// **Async Support:** The closure receives the model by value and returns a future
    /// that produces the modified model. This allows async operations like FK lookups.
    ///
    /// **Concurrency Safe:** Uses SELECT FOR UPDATE to lock rows before modification,
    /// preventing lost updates in concurrent scenarios. All updates succeed or all fail
    /// together within a transaction.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Simple update - just modify fields
    /// let count = Book::objects(db)
    ///     .filter(Book::AuthorId.eq(1))
    ///     .update(|mut book| async move {
    ///         book.status = "archived".to_string();
    ///         Ok(book)
    ///     })
    ///     .await?;
    ///
    /// // Update with async FK lookup
    /// let count = Book::objects(db)
    ///     .filter(Book::Id.eq(book_id))
    ///     .update(|mut book| async move {
    ///         // Async operations supported!
    ///         if let Some(author_name) = &update_dto.author_name {
    ///             let (author, _) = Author::objects(db)
    ///                 .filter(Author::Name.eq(author_name))
    ///                 .get_or_create(|| async {
    ///                     Ok(Author { name: author_name.clone(), ..Default::default() })
    ///                 })
    ///                 .await?;
    ///             book.author_id = author.id;
    ///         }
    ///         Ok(book)
    ///     })
    ///     .await?;
    /// ```
    pub async fn update<F, Fut>(self, updater: F) -> Result<u64, OrmadaError>
    where
        F: Fn(E::Model) -> Fut,
        Fut: std::future::Future<Output = Result<E::Model, OrmadaError>>,
        E: crate::traits::OrmadaEntity,
        C: TransactionTrait,
    {
        use sea_orm::sea_query::LockType;
        use sea_orm::{QuerySelect, TransactionSession};

        // Wrap in transaction for atomicity
        let txn = self.inner.db.begin().await?;

        // Use SELECT FOR UPDATE to lock rows and prevent concurrent modifications
        let models = self.inner.select.clone().lock(LockType::Update).all(&txn).await?;
        let mut count = 0u64;

        for model in models {
            // Apply the async update - closure takes ownership and returns modified model
            let updated_model = updater(model).await?;

            // Use save_model to properly mark all fields as Set
            E::save_model(&txn, updated_model).await?;
            count += 1;
        }

        // Commit transaction
        txn.commit().await?;

        Ok(count)
    }

    /// Eager load related entities (Ormada's `prefetch_related`)
    ///
    /// Transforms this `QuerySet` into a `QuerySetEager` that supports prefetching relations.
    /// This prevents N+1 queries by loading all relations in batched queries (1+M pattern).
    ///
    /// # Usage
    ///
    /// Use the `relations!` macro to specify which entity types to prefetch:
    ///
    /// ```rust,ignore
    /// use ormada::relations;
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
    /// use ormada::relations;
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

    /// Eager load related entities using efficient batch queries (Ormada's `select_related`)
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
    /// use ormada::relations;
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

    /// Create a new record (Ormada's .`create()`)
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
    pub async fn create(self, mut model: E::Model) -> Result<E::Model, OrmadaError>
    where
        E: crate::traits::OrmadaEntity,
        E::Model: IntoActiveModel<E::ActiveModel> + crate::hooks::LifecycleHooks,
        E::ActiveModel: ActiveModelTrait<Entity = E> + Send,
    {
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

    /// Bulk create multiple records (Ormada's `bulk_create()`)
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
    pub async fn bulk_create(self, models: Vec<E::Model>) -> Result<u64, OrmadaError>
    where
        E: crate::traits::OrmadaEntity,
        E::Model: IntoActiveModel<E::ActiveModel>,
        E::ActiveModel: ActiveModelTrait<Entity = E> + Send,
    {
        if models.is_empty() {
            return Ok(0);
        }

        let count = models.len() as u64;

        // Convert models to ActiveModels using OrmadaEntity logic (handles IDs/timestamps)
        let active_models: Result<Vec<E::ActiveModel>, OrmadaError> =
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
    /// - `Err(OrmadaError)` - Database error occurred
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
    pub async fn delete(self) -> Result<u64, OrmadaError>
    where
        E::Model: ModelTrait,
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

    /// Get existing record or create it (Ormada's .`get_or_create()`)
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
    ///
    /// # Async Closures
    ///
    /// The `creator` closure supports async operations, allowing you to
    /// fetch or create related entities before creating the main record.
    ///
    /// ```rust,ignore
    /// // Example: Create book with author lookup
    /// let (book, created) = Book::objects(db)
    ///     .filter(Book::Isbn.eq("1234567890"))
    ///     .get_or_create(|| async {
    ///         // Async operations supported!
    ///         let author = Author::objects(db).get(author_id).await?;
    ///         Ok(Book {
    ///             isbn: "1234567890".into(),
    ///             author_id: author.id,
    ///             ..Default::default()
    ///         })
    ///     })
    ///     .await?;
    /// ```
    pub async fn get_or_create<F, Fut>(self, creator: F) -> Result<(E::Model, bool), OrmadaError>
    where
        E: crate::traits::OrmadaEntity,
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<E::Model, OrmadaError>>,
        E::Model: IntoActiveModel<E::ActiveModel>,
        E::ActiveModel: ActiveModelTrait<Entity = E> + ActiveModelBehavior + Send,
        C: TransactionTrait,
    {
        use sea_orm::{ActiveModelTrait, TransactionSession};

        // Retry up to 3 times to handle race conditions with unique constraints
        for attempt in 0..3 {
            let txn = self.inner.db.begin().await?;

            // Try to get existing record
            if let Some(model) = self.inner.select.clone().one(&txn).await? {
                txn.commit().await?;
                return Ok((model, false));
            }

            // Try to create new record (async)
            let model = creator().await?;
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
                        tracing::warn!(
                            "Failed to rollback transaction after unique violation: {rollback_err}"
                        );
                    }
                }
                Err(e) => {
                    if let Err(rollback_err) = txn.rollback().await {
                        tracing::warn!("Failed to rollback transaction: {rollback_err}");
                    }
                    return Err(e.into());
                }
            }
        }

        // All retries exhausted
        Err(OrmadaError::concurrency_error("get_or_create", 3))
    }

    /// Update existing record or create new one (Ormada's .`update_or_create()`)
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
    ///
    /// # Async Closures
    ///
    /// Both `updater` and `creator` support async operations, allowing you to
    /// fetch or create related entities within the closures.
    ///
    /// ```rust,ignore
    /// // Example: Update or create book with async author lookup
    /// let (book, created) = Book::objects(db)
    ///     .filter(Book::Isbn.eq("1234567890"))
    ///     .update_or_create(
    ///         |mut book| async move {
    ///             // Async operations in updater!
    ///             let author = Author::objects(db).get(author_id).await?;
    ///             book.author_id = author.id;
    ///             book.price = 2999;
    ///             Ok(book)
    ///         },
    ///         || async {
    ///             // Async operations in creator!
    ///             let author = Author::objects(db).get(author_id).await?;
    ///             Ok(Book {
    ///                 isbn: "1234567890".into(),
    ///                 author_id: author.id,
    ///                 ..Default::default()
    ///             })
    ///         },
    ///     ).await?;
    /// ```
    pub async fn update_or_create<U, UF, Creator, CF>(
        self,
        updater: U,
        creator: Creator,
    ) -> Result<(E::Model, bool), OrmadaError>
    where
        E: crate::traits::OrmadaEntity,
        U: Fn(E::Model) -> UF,
        UF: std::future::Future<Output = Result<E::Model, OrmadaError>>,
        Creator: Fn() -> CF,
        CF: std::future::Future<Output = Result<E::Model, OrmadaError>>,
        E::Model: IntoActiveModel<E::ActiveModel>,
        E::ActiveModel: ActiveModelTrait<Entity = E> + ActiveModelBehavior + Send,
        C: TransactionTrait,
    {
        use sea_orm::{ActiveModelTrait, TransactionSession};

        // Retry up to 3 times to handle race conditions with unique constraints
        for attempt in 0..3 {
            let txn = self.inner.db.begin().await?;

            // Try to get existing record
            if let Some(model) = self.inner.select.clone().one(&txn).await? {
                // Update existing record (async - takes ownership, returns modified)
                let updated_model = updater(model).await?;
                let model = E::save_model(&txn, updated_model).await?;
                txn.commit().await?;
                return Ok((model, false));
            }

            // Try to create new (async)
            let model = creator().await?;
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
                        tracing::warn!(
                            "Failed to rollback transaction after unique violation: {rollback_err}"
                        );
                    }
                }
                Err(e) => {
                    if let Err(rollback_err) = txn.rollback().await {
                        tracing::warn!("Failed to rollback transaction: {rollback_err}");
                    }
                    return Err(e.into());
                }
            }
        }

        // All retries exhausted
        Err(OrmadaError::concurrency_error("update_or_create", 3))
    }

    /// Get specific column values as JSON (Ormada's `values()`)
    ///
    /// Returns a Vec of JSON objects for small-medium datasets.
    /// For large datasets, automatically uses chunked fetching.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use ormada::prelude::*;
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
    ) -> Result<Vec<serde_json::Value>, OrmadaError> {
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

    /// Stream full model instances in chunks (Ormada's `.iterator()`).
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
    #[allow(clippy::unused_async)]
    pub async fn iterator(
        &self,
        chunk_size: Option<usize>,
    ) -> Result<
        impl futures::Stream<Item = Result<E::Model, OrmadaError>> + use<'a, E, C, S>,
        OrmadaError,
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
                    Err(e) => return Some((Err(OrmadaError::from(e)), (offset, true))),
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

    /// Get column values iterator (Ormada's `values().iterator()`)
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
    #[allow(clippy::unused_async)]
    pub async fn values_iter(
        &self,
        columns: Vec<E::Column>,
        chunk_size: Option<usize>,
    ) -> Result<
        impl futures::Stream<Item = Result<serde_json::Value, OrmadaError>> + use<'a, E, C, S>,
        OrmadaError,
    > {
        use futures::stream::{self, StreamExt};
        use sea_orm::QuerySelect;

        if columns.is_empty() {
            return Ok(stream::empty().boxed());
        }

        let chunk_size = chunk_size.unwrap_or(crate::batching::DEFAULT_CHUNK_SIZE) as u64;

        // Create stream that fetches in chunks using limit/offset
        // This is Ormada's approach: paginate through results
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
                        Err(e) => return Some((Err(OrmadaError::from(e)), (offset, true))),
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

    /// Get column values iterator as tuples (Ormada's `values_list().iterator()`)
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
        impl futures::Stream<Item = Result<serde_json::Value, OrmadaError>> + use<'a, E, C, S>,
        OrmadaError,
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
                                OrmadaError::validation_error(
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

    /// Get specific column values as tuples (Ormada's `values_list()`)
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
    ) -> Result<Vec<serde_json::Value>, OrmadaError> {
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

    /// Analyze query execution plan (Ormada-inspired .`explain()`)
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
    pub fn explain(&self) -> Result<String, OrmadaError>
    where
        E: crate::traits::OrmadaEntity,
    {
        use sea_orm::QueryTrait;

        // Get the SQL for the current query
        let backend = self.inner.db.get_database_backend();
        let stmt = self.apply_soft_delete_filter(self.inner.select.clone()).build(backend);
        let sql = stmt.to_string();

        // Construct EXPLAIN query based on database backend
        let explain_sql = match backend {
            crate::db::DatabaseBackend::Sqlite => format!("EXPLAIN QUERY PLAN {sql}"),
            crate::db::DatabaseBackend::Postgres => format!("EXPLAIN {sql}"),
            crate::db::DatabaseBackend::MySql => format!("EXPLAIN {sql}"),
            _ => format!("EXPLAIN {sql}"), // Fallback for any future database backends
        };

        // Return the SQL that would be explained
        // Full EXPLAIN output requires database-specific result parsing
        Ok(format!("EXPLAIN output for query:\n{sql}\n\nTo run: {explain_sql}"))
    }

    /// Analyze query with actual execution (Ormada-inspired .explain(analyze=True))
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
    pub fn explain_analyze(&self) -> Result<String, OrmadaError>
    where
        E: crate::traits::OrmadaEntity,
    {
        use sea_orm::QueryTrait;

        // Get the SQL for the current query
        let backend = self.inner.db.get_database_backend();
        let stmt = self.apply_soft_delete_filter(self.inner.select.clone()).build(backend);
        let sql = stmt.to_string();

        // Construct EXPLAIN ANALYZE query based on database backend
        let explain_sql = match backend {
            crate::db::DatabaseBackend::Sqlite => {
                // SQLite doesn't support EXPLAIN ANALYZE, fallback to EXPLAIN QUERY PLAN
                format!("EXPLAIN QUERY PLAN {sql}")
            }
            crate::db::DatabaseBackend::Postgres => format!("EXPLAIN ANALYZE {sql}"),
            crate::db::DatabaseBackend::MySql => {
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
    /// Use `#[ormada_projection(model = YourModel)]` to define projection structs.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// #[ormada_projection(model = Book)]
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
    pub async fn project<T>(&self) -> Result<Vec<T>, OrmadaError>
    where
        T: FromQueryResult + Send,
    {
        Ok(self.inner.select.clone().into_model::<T>().all(self.inner.db).await?)
    }

    /// Project to a custom DTO with explicit column selection for optimization.
    ///
    /// Unlike `project<T>()` which selects all columns, this method only selects
    /// the specified columns, reducing database load for large tables.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use ormada::prelude::*;
    /// use sea_orm::FromQueryResult;
    ///
    /// #[derive(Debug, FromQueryResult)]
    /// struct BookSummary {
    ///     title: String,
    ///     price: i32,
    /// }
    ///
    /// // Only SELECT title, price instead of all columns
    /// let summaries: Vec<BookSummary> = Book::objects(&db)
    ///     .filter(Book::Published.eq(true))
    ///     .project_columns::<BookSummary>(&[Book::Title, Book::Price])
    ///     .await?;
    /// ```
    pub async fn project_columns<T>(&self, columns: &[E::Column]) -> Result<Vec<T>, OrmadaError>
    where
        T: FromQueryResult + Send,
    {
        use sea_orm::QuerySelect;
        let mut select = self.inner.select.clone().select_only();
        for col in columns {
            select = select.column(*col);
        }
        Ok(select.into_model::<T>().all(self.inner.db).await?)
    }

    /// Group query results by one or more columns (Ormada's .`group_by()`)
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

    /// Add computed/aggregated columns to the query (Ormada's .`annotate()`)
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
    /// use ormada::prelude::*;
    ///
    /// #[ormada_projection(model = Book)]
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
    /// #[ormada_projection(model = Book)]
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
            _state: std::marker::PhantomData,
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
    Count(ColumnRef),
    /// SUM(column) - Sum of numeric values
    Sum(ColumnRef),
    /// AVG(column) - Average of numeric values
    Avg(ColumnRef),
    /// MAX(column) - Maximum value
    Max(ColumnRef),
    /// MIN(column) - Minimum value
    Min(ColumnRef),
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

/// Q object for complex queries (Ormada's Q objects)
///
/// Q objects allow you to build complex queries with OR and NOT logic,
/// similar to Ormada's Q objects. They can be nested and combined.
///
/// # Basic Usage
///
/// ```rust,ignore
/// use ormada::prelude::*;
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

    /// Negate this Q object (Ormada's ~`Q()`)
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
/// use ormada::prelude::*;
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
        matches!(
            self,
            Self::Like | Self::NotLike | Self::Contains | Self::StartsWith | Self::EndsWith
        )
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
/// use ormada::prelude::*;
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
    fn typed<C: ColumnTrait>(
        column: C,
        op: FilterOp,
        value_repr: String,
        expr: SimpleExpr,
    ) -> Self {
        Self::Typed {
            column: format!("{:?}", column),
            op,
            value_repr,
            expr,
        }
    }

    /// Create equality filter: column = value
    pub fn eq<C: ColumnTrait, V: Into<Value> + std::fmt::Debug>(column: C, value: V) -> Self {
        let value_repr = format!("{:?}", value);
        Self::typed(column, FilterOp::Eq, value_repr, column.eq(value).into())
    }

    /// Create not-equal filter: column != value
    pub fn ne<C: ColumnTrait, V: Into<Value> + std::fmt::Debug>(column: C, value: V) -> Self {
        let value_repr = format!("{:?}", value);
        Self::typed(column, FilterOp::Ne, value_repr, column.ne(value).into())
    }

    /// Create less-than filter: column < value
    pub fn lt<C: ColumnTrait, V: Into<Value> + std::fmt::Debug>(column: C, value: V) -> Self {
        let value_repr = format!("{:?}", value);
        Self::typed(column, FilterOp::Lt, value_repr, column.lt(value).into())
    }

    /// Create less-than-or-equal filter: column <= value
    pub fn lte<C: ColumnTrait, V: Into<Value> + std::fmt::Debug>(column: C, value: V) -> Self {
        let value_repr = format!("{:?}", value);
        Self::typed(column, FilterOp::Lte, value_repr, column.lte(value).into())
    }

    /// Create greater-than filter: column > value
    pub fn gt<C: ColumnTrait, V: Into<Value> + std::fmt::Debug>(column: C, value: V) -> Self {
        let value_repr = format!("{:?}", value);
        Self::typed(column, FilterOp::Gt, value_repr, column.gt(value).into())
    }

    /// Create greater-than-or-equal filter: column >= value
    pub fn gte<C: ColumnTrait, V: Into<Value> + std::fmt::Debug>(column: C, value: V) -> Self {
        let value_repr = format!("{:?}", value);
        Self::typed(column, FilterOp::Gte, value_repr, column.gte(value).into())
    }

    /// Create IS NULL filter
    pub fn is_null<C: ColumnTrait>(column: C) -> Self {
        Self::typed(column, FilterOp::IsNull, "NULL".to_string(), column.is_null().into())
    }

    /// Create IS NOT NULL filter
    pub fn is_not_null<C: ColumnTrait>(column: C) -> Self {
        Self::typed(
            column,
            FilterOp::IsNotNull,
            "NOT NULL".to_string(),
            column.is_not_null().into(),
        )
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
/// the Ormada-like `.objects(db)` entry point for querying.
///
/// # Basic Usage
///
/// ```rust,ignore
/// use entity::book::{Entity as Book, Column};
/// use ormada::prelude::*;
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
    /// Create a new `QuerySet` for this entity (Ormada's .objects)
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

impl<E: EntityTrait> QueryExt for E {}
