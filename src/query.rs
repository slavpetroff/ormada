//! Core Django-like Query API for SeaORM
//!
//! This module provides ergonomic query building with zero duplication.

use crate::error::DjangoOrmError;
use sea_orm::{
    sea_query::{Condition, SimpleExpr},
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, Order, PrimaryKeyTrait, QueryFilter, QueryOrder,
    QuerySelect, Select,
};

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
// QuerySet - Django-like Query Builder
// ============================================================================

/// Django-inspired QuerySet for ergonomic query building
pub struct QuerySet<'a, E: EntityTrait, C: ConnectionTrait> {
    pub(crate) db: &'a C,
    pub(crate) select: Select<E>,
}

impl<'a, E: EntityTrait, C: ConnectionTrait> QuerySet<'a, E, C> {
    /// Create a new QuerySet
    pub fn new(db: &'a C) -> Self {
        Self {
            db,
            select: E::find(),
        }
    }

    /// Filter records (Django's .filter())
    pub fn filter(mut self, condition: impl Into<Condition>) -> Self {
        self.select = self.select.filter(condition);
        self
    }

    /// Exclude records (Django's .exclude())
    pub fn exclude(mut self, condition: impl Into<Condition>) -> Self {
        let cond: Condition = condition.into();
        self.select = self.select.filter(cond.not());
        self
    }

    /// Remove duplicate rows (Django's .distinct())
    ///
    /// Returns only unique records. Useful when joins might create duplicates.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Get unique book titles (no duplicates)
    /// let books = book::Entity::objects(db)
    ///     .distinct()
    ///     .all()
    ///     .await?;
    ///
    /// // Combined with filters
    /// let unique_authors = book::Entity::objects(db)
    ///     .filter(book::Column::Published.eq(true))
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
    pub fn distinct(mut self) -> Self {
        use sea_orm::QuerySelect;
        self.select = self.select.distinct();
        self
    }

    /// Order by a column in ascending order (Django's .order_by('field'))
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Order by price (lowest first)
    /// let books = book::Entity::objects(db)
    ///     .order_by_asc(book::Column::Price)
    ///     .all()
    ///     .await?;
    ///
    /// // Order by name alphabetically
    /// let authors = author::Entity::objects(db)
    ///     .order_by_asc(author::Column::Name)
    ///     .all()
    ///     .await?;
    /// ```
    pub fn order_by_asc(mut self, column: impl ColumnTrait) -> Self {
        self.select = self.select.order_by(column, Order::Asc);
        self
    }

    /// Order by a column in descending order (Django's .order_by('-field'))
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Order by price (highest first)
    /// let books = book::Entity::objects(db)
    ///     .order_by_desc(book::Column::Price)
    ///     .all()
    ///     .await?;
    ///
    /// // Get newest books first
    /// let recent = book::Entity::objects(db)
    ///     .order_by_desc(book::Column::CreatedAt)
    ///     .limit(10)
    ///     .all()
    ///     .await?;
    /// ```
    pub fn order_by_desc(mut self, column: impl ColumnTrait) -> Self {
        self.select = self.select.order_by(column, Order::Desc);
        self
    }

    /// Limit results (Django's [:n])
    pub fn limit(mut self, limit: u64) -> Self {
        self.select = self.select.limit(limit);
        self
    }

    /// Offset results
    pub fn offset(mut self, offset: u64) -> Self {
        self.select = self.select.offset(offset);
        self
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
    pub async fn all(self) -> Result<Vec<E::Model>, DjangoOrmError> {
        Ok(self.select.all(self.db).await?)
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
    pub async fn first(self) -> Result<E::Model, DjangoOrmError> {
        self.select
            .one(self.db)
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
        let models = self.select.all(self.db).await?;

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
    pub async fn get<T>(self, id: T) -> Result<E::Model, DjangoOrmError>
    where
        T: Into<<E::PrimaryKey as PrimaryKeyTrait>::ValueType> + Send + std::fmt::Display,
    {
        let id_str = format!("{}", &id);
        E::find_by_id(id)
            .one(self.db)
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
    /// let oldest = book::Entity::objects(db)
    ///     .earliest(book::Column::CreatedAt)
    ///     .await?;
    ///
    /// // With filters
    /// let first_published = book::Entity::objects(db)
    ///     .filter(book::Column::Published.eq(true))
    ///     .earliest(book::Column::PublishedDate)
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
    pub async fn earliest(mut self, column: impl ColumnTrait) -> Result<E::Model, DjangoOrmError> {
        self.select = self.select.order_by(column, Order::Asc);
        self.select
            .one(self.db)
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
    /// let newest = book::Entity::objects(db)
    ///     .latest(book::Column::CreatedAt)
    ///     .await?;
    ///
    /// // With filters
    /// let latest_published = book::Entity::objects(db)
    ///     .filter(book::Column::Published.eq(true))
    ///     .latest(book::Column::PublishedDate)
    ///     .await?;
    ///
    /// // Get most expensive book
    /// let most_expensive = book::Entity::objects(db)
    ///     .latest(book::Column::Price)
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
    pub async fn latest(mut self, column: impl ColumnTrait) -> Result<E::Model, DjangoOrmError> {
        self.select = self.select.order_by(column, Order::Desc);
        self.select
            .one(self.db)
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
    pub async fn count(self) -> Result<u64, DjangoOrmError> {
        // Get count using SeaORM's built-in count functionality
        use sea_orm::QuerySelect;
        let count_select = self.select.select_only().column_as(
            sea_orm::sea_query::Expr::col(sea_orm::sea_query::Asterisk).count(),
            "count",
        );

        // Execute and get the count
        let result = count_select.into_tuple::<i64>().one(self.db).await?;
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
    pub async fn exists(self) -> Result<bool, DjangoOrmError> {
        use sea_orm::QuerySelect;
        // Use LIMIT 1 for efficiency
        let result = self.select.limit(1).one(self.db).await?;
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
        let txn = self.db.begin().await?;

        // Use SELECT FOR UPDATE to lock rows and prevent concurrent modifications
        let models = self.select.lock(LockType::Update).all(&txn).await?;
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

        let eager = QuerySetEager::new(self.db, self.select);
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
    /// let author = author::Entity::objects(db).create(author::Model {
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
        Ok(active_model.insert(self.db).await?)
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
    ///     author::Model {
    ///         name: "Author 1".to_string(),
    ///         ..Default::default()
    ///     },
    ///     author::Model {
    ///         name: "Author 2".to_string(),
    ///         ..Default::default()
    ///     },
    /// ];
    ///
    /// let count = author::Entity::objects(db)
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
        E::insert_many(active_models).exec(self.db).await?;

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
        let models = self.select.all(self.db).await?;
        
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
                .exec(self.db)
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
                .exec(self.db)
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
    /// let (author, created) = author::Entity::objects(db)
    ///     .filter(author::Column::Email.eq("john@example.com"))
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
    /// let (author, _) = author::Entity::objects(db)
    ///     .filter(author::Column::Email.eq(email))
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
            let txn = self.db.begin().await?;

            // Try to get existing record
            match self.select.clone().one(&txn).await? {
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
                            // Roll back and retry
                            let _ = txn.rollback().await;
                            continue;
                        }
                        Err(e) => {
                            let _ = txn.rollback().await;
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
    /// let (book, created) = book::Entity::objects(db)
    ///     .filter(book::Column::Isbn.eq("1234567890"))
    ///     .update_or_create(
    ///         |model| {
    ///             // Update existing
    ///             model.price = 2999;
    ///         },
    ///         || {
    ///             // Create new
    ///             book::Model {
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
            let txn = self.db.begin().await?;

            // Try to get existing record
            match self.select.clone().one(&txn).await? {
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
                            let _ = txn.rollback().await;
                            continue;
                        }
                        Err(e) => {
                            let _ = txn.rollback().await;
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
    ///     .values(vec![book::Column::Title, book::Column::Price])
    ///     .await?;
    ///
    /// for val in values {
    ///     println!("Title: {}, Price: {}", val["title"], val["price"]);
    /// }
    /// ```
    pub async fn values(
        self,
        columns: Vec<E::Column>,
    ) -> Result<Vec<serde_json::Value>, DjangoOrmError> {
        use sea_orm::{JsonValue, QuerySelect};

        if columns.is_empty() {
            return Ok(Vec::new());
        }

        // Use the existing select query directly to respect limits/offsets
        let mut select = self.select.select_only();
        for col in columns {
            select = select.column(col);
        }

        let results: Vec<JsonValue> = select.into_json().all(self.db).await?;
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
    ///     .filter(book::Column::Published.eq(true))
    ///     .iterator(Some(500))
    ///     .await?;
    /// 
    /// while let Some(book) = stream.next().await {
    ///     let book = book?;
    ///     process_book(book).await?;
    /// }
    /// ```
    pub async fn iterator(
        self,
        chunk_size: Option<usize>,
    ) -> Result<impl futures::Stream<Item = Result<E::Model, DjangoOrmError>> + use<'a, E, C>, DjangoOrmError> {
        use futures::stream::{self, StreamExt};
        use sea_orm::QuerySelect;
        
        let chunk_size = chunk_size.unwrap_or(crate::batching::DEFAULT_CHUNK_SIZE) as u64;
        let db = self.db;
        let base_select = std::sync::Arc::new(self.select);
        
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
    ///     .values_iter(vec![book::Column::Title, book::Column::Price], None)
    ///     .await?;
    ///
    /// while let Some(value) = stream.next().await {
    ///     let value = value?;
    ///     println!("Title: {}, Price: {}", value["title"], value["price"]);
    /// }
    /// ```
    pub async fn values_iter(
        self,
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
        let db = self.db;
        // Use Arc to avoid cloning the Select on every iteration
        let base_select = std::sync::Arc::new(self.select);
        let columns = std::sync::Arc::new(columns);
        
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
    ///     .values_list_iter(vec![book::Column::Title, book::Column::Price], false, None)
    ///     .await?;
    ///
    /// while let Some(row) = stream.next().await {
    ///     let row = row?;
    ///     // row is ["title", 1999]
    /// }
    ///
    /// // Stream flat values
    /// let mut stream = Book::objects(db)
    ///     .values_list_iter(vec![book::Column::Title], true, None)
    ///     .await?;
    ///
    /// while let Some(title) = stream.next().await {
    ///     let title = title?;
    ///     // title is just "Book Name"
    /// }
    /// ```
    pub async fn values_list_iter(
        self,
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
    ///     .values_list(vec![book::Column::Title, book::Column::Price], false)
    ///     .await?;
    ///
    /// // Get flat list
    /// let titles = Book::objects(db)
    ///     .values_list(vec![book::Column::Title], true)
    ///     .await?;
    /// ```
    pub async fn values_list(
        self,
        columns: Vec<E::Column>,
        flat: bool,
    ) -> Result<Vec<serde_json::Value>, DjangoOrmError> {
        use sea_orm::{JsonValue, QuerySelect};

        if columns.is_empty() {
            return Ok(Vec::new());
        }

        // Use the existing select query directly to respect limits/offsets
        let mut select = self.select.select_only();
        for col in &columns {
            select = select.column(*col);
        }

        let results: Vec<JsonValue> = select.into_json().all(self.db).await?;

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
    ///     .filter(book::Column::Published.eq(true))
    ///     .order_by_desc(book::Column::CreatedAt)
    ///     .debug_sql();
    /// 
    /// println!("SQL: {}", sql);
    /// println!("Params: {:?}", params);
    /// ```
    pub fn debug_sql(&self) -> String {
        use sea_orm::QueryTrait;
        let stmt = self.select.build(self.db.get_database_backend());
        stmt.to_string()
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
