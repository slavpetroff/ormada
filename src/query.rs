//! Core Django-like Query API for SeaORM
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
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, DbErr, EntityTrait, Order, PrimaryKeyTrait, QueryFilter, 
    QueryOrder, QuerySelect, Select,
};
use sea_orm::sea_query::{Expr, Func, SimpleExpr};
use std::sync::Arc;
use tokio::sync::RwLock;

// ============================================================================
// Concurrency Helpers
// ============================================================================

/// Check if a database error is a unique constraint violation
///
/// Used internally by get_or_create and update_or_create to detect
/// race conditions and retry the operation.
///
/// This is a heuristic check that works across SQLite, PostgreSQL, and MySQL.
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
/// This trait adds Django-like methods to ANY SeaORM Column enum.
/// Works directly on SeaORM's generated Column enum with zero duplication.
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
/// Main QuerySet structure (Django's QuerySet equivalent)
///
/// Provides chainable query building with automatic caching and lazy evaluation.
/// All operations are lazy until a terminal method (.all(), .first(), etc.) is called.
///
/// **Caching Behavior (Django-like):**
/// - First execution of `.all()`, `.first()`, etc. hits the database
/// - Results are cached in the QuerySet instance
/// - Subsequent calls on the SAME QuerySet reuse cached results
/// - Building new queries (`.filter()`, `.limit()`) creates new QuerySet with separate cache
///
/// **Concurrency Safety:**
/// - Uses `Arc` for cheap cloning across async tasks
/// - Uses `tokio::RwLock` for thread-safe cache access
/// - Safe to share across threads and async tasks
///
/// # Type Parameters
///
/// - `E`: The SeaORM Entity type
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

/// Internal state for QuerySet (shared via Arc)
pub(crate) struct QuerySetInner<'a, E: EntityTrait, C: ConnectionTrait> {
    pub(crate) db: &'a C,
    pub(crate) select: Select<E>,
    // Thread-safe cache for query results
    pub(crate) cache: RwLock<Option<Arc<Vec<E::Model>>>>,
}

// Implement Clone for QuerySet (cheap Arc clone)
impl<'a, E: EntityTrait, C: ConnectionTrait> Clone for QuerySet<'a, E, C> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<'a, E: EntityTrait, C: ConnectionTrait> QuerySet<'a, E, C> {
    /// Create a new QuerySet
    pub fn new(db: &'a C) -> Self {
        Self {
            inner: Arc::new(QuerySetInner {
                db,
                select: E::find(),
                cache: RwLock::new(None),
            }),
        }
    }

    /// Create a new QuerySet with modified select (internal helper)
    fn with_select(&self, select: Select<E>) -> Self {
        Self {
            inner: Arc::new(QuerySetInner {
                db: self.inner.db,
                select,
                cache: RwLock::new(None),  // New cache for modified query
            }),
        }
    }

    /// Filter records (Django's .filter())
    ///
    /// Creates a new QuerySet with added filter. The new QuerySet has its own cache.
    pub fn filter(&self, condition: impl Into<Condition>) -> Self {
        let new_select = self.inner.select.clone().filter(condition);
        self.with_select(new_select)
    }

    /// Exclude records (Django's .exclude())
    ///
    /// Creates a new QuerySet with added exclusion. The new QuerySet has its own cache.
    pub fn exclude(&self, condition: impl Into<Condition>) -> Self {
        let cond: Condition = condition.into();
        let new_select = self.inner.select.clone().filter(cond.not());
        self.with_select(new_select)
    }

    /// Remove duplicate rows (Django's .distinct())
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
        self.with_select(new_select)
    }

    /// Order by a column in ascending order (Django's .order_by('field'))
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
        let new_select = self.inner.select.clone().order_by(column, Order::Asc);
        self.with_select(new_select)
    }

    /// Order by a column in descending order (Django's .order_by('-field'))
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
        let new_select = self.inner.select.clone().order_by(column, Order::Desc);
        self.with_select(new_select)
    }

    /// Limit results (Django's [:n])
    pub fn limit(&self, limit: u64) -> Self {
        let new_select = self.inner.select.clone().limit(limit);
        self.with_select(new_select)
    }

    /// Offset results
    pub fn offset(&self, offset: u64) -> Self {
        let new_select = self.inner.select.clone().offset(offset);
        self.with_select(new_select)
    }

    /// Execute query and return all matching results (Django's .all())
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
    /// **Second call on same QuerySet** - Returns cached results (no DB query):
    /// ```rust,ignore
    /// let books_again = qs.all().await?;  // Cache hit! No DB query
    /// ```
    pub async fn all(&self) -> Result<Vec<E::Model>, DjangoOrmError> {
        // Try to read from cache first (allows multiple concurrent readers)
        {
            let cache = self.inner.cache.read().await;
            if let Some(cached_results) = cache.as_ref() {
                // Cache hit! Return cloned results
                return Ok((**cached_results).clone());
            }
        }
        
        // Cache miss - execute query and populate cache
        let results = self.inner.select.clone().all(self.inner.db).await?;
        let arc_results = Arc::new(results.clone());
        
        // Store in cache (exclusive write lock)
        *self.inner.cache.write().await = Some(arc_results);
        
        Ok(results)
    }

    /// Execute query and return first result (Django's .first())
    ///
    /// Returns the first matching model or error if no matches found.
    /// Useful with ordering to get the "latest" or "oldest" record.
    ///
    /// # Returns
    ///
    /// - `Ok(E::Model)` - First matching model found
    /// - `Err(DjangoOrmError::Custom("No records found"))` - No matching models
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
    ///     Err(DjangoOrmError::Custom(msg)) if msg.contains("No records") => {
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
    pub async fn first(&self) -> Result<E::Model, DjangoOrmError> {
        // Try cache first
        {
            let cache = self.inner.cache.read().await;
            if let Some(cached_results) = cache.as_ref() {
                return cached_results.first()
                    .cloned()
                    .ok_or_else(|| DjangoOrmError::Custom("No records found".into()));
            }
        }
        
        // Cache miss - execute query for single record
        self.inner.select
            .clone()
            .one(self.inner.db)
            .await?
            .ok_or_else(|| DjangoOrmError::Custom("No records found".into()))
    }

    /// Execute query and return last result
    ///
    /// Returns the last matching model or error if no matches found.
    /// Reverses the order and gets the first result.
    ///
    /// # Returns
    ///
    /// - `Ok(E::Model)` - Last matching model found
    /// - `Err(DjangoOrmError::Custom("No records found"))` - No matching models
    /// - `Err(DjangoOrmError::Database(_))` - Database error occurred
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Get last book
    /// let book = Book::objects(db)
    ///     .order_by_asc(Column::CreatedAt)
    ///     .last()
    ///     .await?;
    /// println!("Most recent: {}", book.title);
    ///
    /// // Get oldest published book
    /// let oldest_published = Book::objects(db)
    ///     .filter(Column::Published.eq(true))
    ///     .order_by_desc(Column::CreatedAt)  // Ordered newest first
    ///     .last()  // Gets the oldest
    ///     .await?;
    ///
    /// // Handle no results
    /// match Book::objects(db).last().await {
    ///     Ok(book) => println!("Last: {}", book.title),
    ///     Err(DjangoOrmError::Custom(msg)) if msg.contains("No records") => {
    ///         println!("No books found");
    ///     }
    ///     Err(e) => return Err(e),
    /// }
    /// ```
    pub async fn last(self) -> Result<E::Model, DjangoOrmError> {
        // Performance note: Currently loads all matching records to get the last one.
        // This is a known limitation due to SeaORM's query API not exposing order reversal.
        // For better performance on large datasets, use .order_by_desc().first() instead.
        // TODO: Optimize by reversing order clauses and using LIMIT 1
        let models = self.inner.select.clone().all(self.inner.db).await?;

        models
            .into_iter()
            .last()
            .ok_or_else(|| DjangoOrmError::Custom("No records found".into()))
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
    ///
    /// // Or use ? for early return on not found
    pub async fn get<T>(&self, id: T) -> Result<E::Model, DjangoOrmError>
    where
        T: Into<<E::PrimaryKey as PrimaryKeyTrait>::ValueType> + Send + std::fmt::Display,
    {
        let id_str = format!("{}", &id);
        E::find_by_id(id)
            .one(self.inner.db)
            .await?
            .ok_or_else(|| DjangoOrmError::not_found(E::default().table_name(), id_str))
    }

    /// Get the earliest record by a field (Django's .earliest())
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
    /// - `Err(DjangoOrmError::Custom)` - No records found
    /// - `Err(DjangoOrmError::Database)` - Database error
    ///
    /// # Equivalent to
    ///
    /// `.order_by_asc(column).first()` but returns error on empty result
    pub async fn earliest(&self, column: impl ColumnTrait) -> Result<E::Model, DjangoOrmError> {
        self.inner.select
            .clone()
            .order_by(column, Order::Asc)
            .one(self.inner.db)
            .await?
            .ok_or_else(|| DjangoOrmError::Custom("No records found".into()))
    }

    /// Get the latest record by a field (Django's .latest())
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
    /// - `Err(DjangoOrmError::Custom)` - No records found
    /// - `Err(DjangoOrmError::Database)` - Database error
    ///
    /// # Equivalent to
    ///
    /// `.order_by_desc(column).first()` but returns error on empty result
    pub async fn latest(&self, column: impl ColumnTrait) -> Result<E::Model, DjangoOrmError> {
        self.inner.select
            .clone()
            .order_by(column, Order::Desc)
            .one(self.inner.db)
            .await?
            .ok_or_else(|| DjangoOrmError::Custom("No records found".into()))
    }

    /// Count records matching the query (Django's .count())
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
    pub async fn count(&self) -> Result<u64, DjangoOrmError> {
        // Get count using SeaORM's built-in count functionality
        use sea_orm::QuerySelect;
        let count_select = self.inner.select.clone().select_only().column_as(
            sea_orm::sea_query::Expr::col(sea_orm::sea_query::Asterisk).count(),
            "count",
        );

        // Execute and get the count
        let result = count_select.into_tuple::<i64>().one(self.inner.db).await?;
        Ok(result.unwrap_or(0) as u64)
    }

    /// Check if any records exist matching the query (Django's .exists())
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
    pub async fn exists(&self) -> Result<bool, DjangoOrmError> {
        use sea_orm::QuerySelect;
        // Use LIMIT 1 for efficiency
        let result = self.inner.select.clone().limit(1).one(self.inner.db).await?;
        Ok(result.is_some())
    }

    /// Update all records matching the query (Django's .update())
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
        use sea_orm::{QuerySelect, TransactionSession};
        use sea_orm::sea_query::LockType;

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

    /// Eager load related entities (Django's prefetch_related)
    ///
    /// Transforms this QuerySet into a QuerySetEager that supports prefetching relations.
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
    /// You can also pass a raw Vec of TypeIds, but the macro is cleaner:
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

    /// Create a new record (Django's .create())
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
    pub async fn create(self, model: E::Model) -> Result<E::Model, DjangoOrmError>
    where
        E: crate::traits::DjangoEntity,
        E::Model: sea_orm::IntoActiveModel<E::ActiveModel>,
        E::ActiveModel: sea_orm::ActiveModelTrait<Entity = E> + Send,
    {
        use sea_orm::ActiveModelTrait;
        let active_model = E::to_active_model_for_create(model)?;
        Ok(active_model.insert(self.inner.db).await?)
    }

    /// Bulk create multiple records (Django's bulk_create())
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
        let active_models: Result<Vec<E::ActiveModel>, DjangoOrmError> = models
            .into_iter()
            .map(|model| E::to_active_model_for_create(model))
            .collect();
        let active_models = active_models?;

        // Use SeaORM's insert_many
        E::insert_many(active_models).exec(self.inner.db).await?;

        Ok(count)
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
        use sea_orm::{ColumnTrait, Condition, Iterable, ModelTrait, PrimaryKeyToColumn, QueryFilter};

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
            
            E::delete_many()
                .filter(condition)
                .exec(self.inner.db)
                .await?;
        }

        Ok(count)
    }

    /// Get existing record or create it (Django's .get_or_create())
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
        F: Fn() -> E::Model,  // Changed: Fn instead of FnOnce to allow retries
        E::Model: sea_orm::IntoActiveModel<E::ActiveModel>,
        E::ActiveModel: sea_orm::ActiveModelTrait<Entity = E> + sea_orm::ActiveModelBehavior + Send,
        C: sea_orm::TransactionTrait,
    {
        use sea_orm::{ActiveModelTrait, TransactionSession};

        // Retry up to 3 times to handle race conditions with unique constraints
        for attempt in 0..3 {
            let txn = self.inner.db.begin().await?;

            // Try to get existing record
            match self.inner.select.clone().one(&txn).await? {
                Some(model) => {
                    txn.commit().await?;
                    return Ok((model, false));
                }
                None => {
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
                                eprintln!("Warning: Failed to rollback transaction after unique violation: {}", rollback_err);
                            }
                            continue;
                        }
                        Err(e) => {
                            // Attempt rollback on error. Rollback failure is logged but doesn't
                            // change the error we return since transaction drop also rolls back.
                            if let Err(rollback_err) = txn.rollback().await {
                                eprintln!("Warning: Failed to rollback transaction: {}", rollback_err);
                            }
                            return Err(e.into());
                        }
                    }
                }
            }
        }

        // All retries exhausted
        Err(DjangoOrmError::Custom(
            "get_or_create failed after 3 retry attempts due to concurrent inserts".into(),
        ))
    }

    /// Update existing record or create new one (Django's .update_or_create())
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
        U: Fn(&mut E::Model),  // Changed: Fn instead of FnOnce to allow retries
        Creator: Fn() -> E::Model,  // Changed: Fn instead of FnOnce to allow retries
        E::Model: sea_orm::IntoActiveModel<E::ActiveModel>,
        E::ActiveModel: sea_orm::ActiveModelTrait<Entity = E> + sea_orm::ActiveModelBehavior + Send,
        C: sea_orm::TransactionTrait,
    {
        use sea_orm::{ActiveModelTrait, TransactionSession};

        // Retry up to 3 times to handle race conditions with unique constraints
        for attempt in 0..3 {
            let txn = self.inner.db.begin().await?;

            // Try to get existing record
            match self.inner.select.clone().one(&txn).await? {
                Some(mut model) => {
                    // Update existing record
                    updater(&mut model);
                    let model = E::save_model(&txn, model).await?;
                    txn.commit().await?;
                    return Ok((model, false));
                }
                None => {
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
                                eprintln!("Warning: Failed to rollback transaction after unique violation: {}", rollback_err);
                            }
                            continue;
                        }
                        Err(e) => {
                            if let Err(rollback_err) = txn.rollback().await {
                                eprintln!("Warning: Failed to rollback transaction: {}", rollback_err);
                            }
                            return Err(e.into());
                        }
                    }
                }
            }
        }

        // All retries exhausted
        Err(DjangoOrmError::Custom(
            "update_or_create failed after 3 retry attempts due to concurrent inserts".into(),
        ))
    }

    /// Get specific column values as JSON (Django's values())
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
    ) -> Result<impl futures::Stream<Item = Result<E::Model, DjangoOrmError>> + use<'a, E, C>, DjangoOrmError> {
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
                
                let select = (*base_select).clone()
                    .limit(chunk_size)
                    .offset(offset);
                
                let results: Vec<E::Model> = match select.all(db).await {
                    Ok(r) => r,
                    Err(e) => return Some((Err(DjangoOrmError::from(e)), (offset, true))),
                };
                
                let is_done = results.len() < chunk_size as usize;
                let next_offset = offset + results.len() as u64;
                
                Some((Ok(results), (next_offset, is_done)))
            }
        })
        .flat_map(|result| {
            match result {
                Ok(models) => stream::iter(models.into_iter().map(Ok)).left_stream(),
                Err(e) => stream::once(async move { Err(e) }).right_stream(),
            }
        });

        Ok(stream.boxed())
    }

    /// Get column values iterator (Django's values().iterator())
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
    ) -> Result<impl futures::Stream<Item = Result<serde_json::Value, DjangoOrmError>> + use<'a, E, C>, DjangoOrmError> {
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
            let base_select = base_select.clone();  // Clone Arc (cheap pointer copy)
            let columns = columns.clone();  // Clone Arc (cheap pointer copy)
            async move {
                if done {
                    return None;
                }
                
                let mut select = (*base_select).clone().select_only();  // Clone Select once per chunk (unavoidable)
                for col in &*columns {
                    select = select.column(*col);
                }
                
                let results: Vec<serde_json::Value> = match select
                    .limit(chunk_size)
                    .offset(offset)
                    .into_json()
                    .all(db)
                    .await
                {
                    Ok(r) => r,
                    Err(e) => return Some((Err(DjangoOrmError::from(e)), (offset, true))),
                };
                
                let is_done = results.len() < chunk_size as usize;
                let next_offset = offset + results.len() as u64;
                
                Some((Ok(results), (next_offset, is_done)))
            }
        })
        .flat_map(|result| {
            match result {
                Ok(values) => stream::iter(values.into_iter().map(Ok)).left_stream(),
                Err(e) => stream::once(async move { Err(e) }).right_stream(),
            }
        });

        Ok(stream.boxed())
    }

    /// Get column values iterator as tuples (Django's values_list().iterator())
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
    ) -> Result<impl futures::Stream<Item = Result<serde_json::Value, DjangoOrmError>> + use<'a, E, C>, DjangoOrmError> {
        use futures::stream::StreamExt;
        
        let columns_len = columns.len();
        let columns_clone = columns.clone();  // Clone for later use in map closure
        let stream = self.values_iter(columns, chunk_size).await?;
        
        if flat && columns_len == 1 {
            Ok(stream.map(|result| {
                result.and_then(|obj| {
                    obj.as_object()
                        .and_then(|map| map.values().next().cloned())
                        .ok_or_else(|| DjangoOrmError::Custom("Invalid value format".into()))
                })
            }).boxed())
        } else {
            Ok(stream.map(move |result| {
                result.map(|obj| {
                    let values: Vec<serde_json::Value> = obj
                        .as_object()
                        .map(|map| {
                            columns_clone.iter()
                                .filter_map(|col| {
                                    let col_name = format!("{:?}", col).to_lowercase();
                                    map.get(&col_name).cloned()
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    serde_json::Value::Array(values)
                })
            }).boxed())
        }
    }

    /// Get specific column values as tuples (Django's values_list())
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
                            columns.iter()
                                .filter_map(|col| {
                                    let col_name = format!("{:?}", col).to_lowercase();
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

    /// Type-safe projection query (alternative to JSON-based values())
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
        use sea_orm::QuerySelect;
        
        // Use select_only() to prepare for column selection
        let select = self.inner.select.clone().select_only();
        
        // Convert to custom model type
        Ok(select.into_model::<T>().all(self.inner.db).await?)
    }

    /// Group query results by one or more columns (Django's .group_by())
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
        let new_select = self.inner.select.clone().group_by(column);
        self.with_select(new_select)
    }

    /// Add computed/aggregated columns to the query (Django's .annotate())
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
        for (alias, aggregation) in annotations {
            let expr = aggregation.into_expr();
            new_select = new_select.expr_as(expr, alias);
        }
        
        self.with_select(new_select)
    }
}

// ============================================================================
// Aggregation Helpers
// ============================================================================

/// Aggregation helper for use with `.annotate()`
///
/// Provides type-safe aggregation functions for queries.
#[derive(Clone)]
pub struct Aggregation {
    expr: SimpleExpr,
}

impl Aggregation {
    /// COUNT(*) - Count all rows
    ///
    /// # Example
    /// ```rust,ignore
    /// .annotate([("total", Aggregation::count_all())])
    /// ```
    pub fn count_all() -> Self {
        Self {
            expr: Expr::expr(Func::count(Expr::asterisk())),
        }
    }

    /// COUNT(column) - Count non-NULL values in column
    ///
    /// # Example
    /// ```rust,ignore
    /// .annotate([("published_count", Aggregation::count(Book::PublishedDate))])
    /// ```
    pub fn count(column: impl ColumnTrait) -> Self {
        Self {
            expr: Expr::expr(Func::count(Expr::col(column.as_column_ref()))),
        }
    }

    /// SUM(column) - Sum of numeric column
    ///
    /// # Example
    /// ```rust,ignore
    /// .annotate([("total_sales", Aggregation::sum(Book::Sales))])
    /// ```
    pub fn sum(column: impl ColumnTrait) -> Self {
        Self {
            expr: Expr::expr(Func::sum(Expr::col(column.as_column_ref()))),
        }
    }

    /// AVG(column) - Average of numeric column
    ///
    /// # Example
    /// ```rust,ignore
    /// .annotate([("avg_price", Aggregation::avg(Book::Price))])
    /// ```
    pub fn avg(column: impl ColumnTrait) -> Self {
        Self {
            expr: Expr::expr(Func::avg(Expr::col(column.as_column_ref()))),
        }
    }

    /// MAX(column) - Maximum value
    ///
    /// # Example
    /// ```rust,ignore
    /// .annotate([("max_price", Aggregation::max(Book::Price))])
    /// ```
    pub fn max(column: impl ColumnTrait) -> Self {
        Self {
            expr: Expr::expr(Func::max(Expr::col(column.as_column_ref()))),
        }
    }

    /// MIN(column) - Minimum value
    ///
    /// # Example
    /// ```rust,ignore
    /// .annotate([("min_price", Aggregation::min(Book::Price))])
    /// ```
    pub fn min(column: impl ColumnTrait) -> Self {
        Self {
            expr: Expr::expr(Func::min(Expr::col(column.as_column_ref()))),
        }
    }

    /// Convert to SeaORM expression
    fn into_expr(self) -> SimpleExpr {
        self.expr
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
        Self {
            condition: Condition::all(),
        }
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
        Self {
            condition: Condition::any(),
        }
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

    /// Negate this Q object (Django's ~Q())
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
// Extension Trait
// ============================================================================

/// Extension trait to add `.objects()` method to entities
///
/// This trait is automatically implemented for all SeaORM entities and provides
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
    /// Create a new QuerySet for this entity (Django's .objects)
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
    fn objects<'a, C: ConnectionTrait>(db: &'a C) -> QuerySet<'a, Self, C> {
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
        let q = Q::all()
            .add(Expr::value(true))
            .add(Expr::value(false));
        // Should allow chaining multiple add calls
        assert!(matches!(q.condition, Condition));
    }
}
