#![allow(clippy::doc_markdown)]
#![allow(clippy::future_not_send)]
#![allow(clippy::cast_sign_loss)]

//! Relation loading for Django-like ORM
//!
//! This module provides eager loading of relations with zero N+1 queries.
//!
//! ## Design Philosophy
//!
//! Instead of wrapping models, we generate `ModelWithRelations` structs that have:
//! - All original model fields as direct properties
//! - Relation fields as `Option<RelatedModel>`
//!
//! This is achieved via the `#[derive(OrmadaModel)]` macro which reads `SeaORM`'s
//! Relation definitions and generates the extended struct at compile time.

use crate::db::ConnectionTrait;
use crate::error::OrmadaError;
use crate::fields::{ColumnTrait, Condition};
use crate::models::{EntityTrait, QueryFilter, QueryOrder, QuerySelect, Select};
use rustc_hash::FxHashMap;
use sea_orm::sea_query::{Asterisk, Expr, SimpleExpr};

// /// Note: RelationGraph has been removed in favor of compile-time typed relations
// See LoadRelations trait and HasRelation trait for the new zero-cost approach

// ============================================================================
// Relations Macro
// ============================================================================

/// Helper macro to create relation specifications for prefetching
///
/// This macro provides a clean syntax for specifying which relations to prefetch.
/// Users just pass the Model type (e.g., `Author`) and the macro extracts the Entity.
///
/// # Examples
///
/// ```rust,ignore
/// use ormada::relations;
///
/// // Single relation - just use the Model name!
/// let books = Book::objects(db)
///     .prefetch_related(relations![Author])
///     .all()
///     .await?;
///
/// // Multiple relations
/// let books = Book::objects(db)
///     .prefetch_related(relations![Author, Publisher, Category])
///     .all()
///     .await?;
/// ```
#[macro_export]
macro_rules! relations {
    ($model:ty) => {
        $crate::relations::RelationSpec::<< $model as $crate::relations::HasEntityType >::__Entity>::new()
    };
    ($($model:ty),+ $(,)?) => {
        ( $( $crate::relations::RelationSpec::<< $model as $crate::relations::HasEntityType >::__Entity>::new() ),+ )
    };
}

/// Typed relation specification - zero runtime cost
pub struct RelationSpec<E: EntityTrait> {
    _marker: std::marker::PhantomData<E>,
}

impl<E: EntityTrait> RelationSpec<E> {
    /// Create a new relation specification (zero-cost, compile-time only)
    pub const fn new() -> Self {
        Self { _marker: std::marker::PhantomData }
    }
}

impl<E: EntityTrait> Default for RelationSpec<E> {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Relation Loading Traits
// ============================================================================

/// Trait to extract Entity type from Model (for relations! macro)
///
/// This is automatically implemented by the #[ormada_model] macro
#[doc(hidden)]
pub trait HasEntityType {
    /// The SeaORM Entity type for this Model
    type __Entity: EntityTrait;
}

/// Trait for entities that can load a specific relation
pub trait HasRelation<Related: EntityTrait>:
    EntityTrait + crate::traits::WithRelationsTrait
{
    /// The type of the foreign key (usually i32, but can be other types)
    type RelatedPK: std::cmp::Eq + std::hash::Hash + Clone;

    /// Extract the foreign key from a model
    fn get_foreign_key(model: &<Self as EntityTrait>::Model) -> Self::RelatedPK;

    /// Load related models for a batch of parent models
    /// This must be implemented by the macro since we need access to the specific Column enum
    async fn load_related<C: ConnectionTrait>(
        models: &[<Self as EntityTrait>::Model],
        db: &C,
    ) -> Result<FxHashMap<Self::RelatedPK, Related::Model>, OrmadaError>;

    /// Set the related model on the ModelWithRelations wrapper
    /// This is called during prefetch_related to populate relation fields
    fn set_related(
        model: &mut <Self as crate::traits::WithRelationsTrait>::ModelWithRelations,
        related: Option<Related::Model>,
    );

    /// Get the relation definition for JOIN-based loading
    ///
    /// This is used internally by `select_related` for efficient single-query JOINs.
    /// The `#[ormada_model]` macro generates this automatically for FK relations.
    #[doc(hidden)]
    #[allow(clippy::unimplemented)]
    fn relation_def() -> crate::__internal::RelationDef {
        unimplemented!("relation_def not implemented for this relation. The #[ormada_model] macro should generate this.")
    }
}

/// Trait for JOIN-based loading of relations (select_related)
///
/// This trait enables single-query loading using SQL JOINs.
/// Unlike `LoadRelations` which uses separate queries, this uses JOINs for efficiency.
pub trait JoinLoadRelations<Parent: EntityTrait + crate::traits::WithRelationsTrait> {
    /// Execute the JOIN query and return models with relations populated
    async fn load_with_join<C: ConnectionTrait>(
        select: Select<Parent>,
        db: &C,
    ) -> Result<Vec<Parent::ModelWithRelations>, OrmadaError>;

    /// Build the SQL string for the JOIN query (for debugging/explain)
    fn build_join_sql<C: ConnectionTrait>(select: &Select<Parent>, db: &C) -> String;
}

/// Trait for loading relations at compile time
pub trait LoadRelations<Parent: EntityTrait + crate::traits::WithRelationsTrait> {
    /// The type of data loaded (tuple of `HashMaps`)
    type Output;

    /// Load all relations
    async fn load_all<C: ConnectionTrait>(
        models: &[<Parent as EntityTrait>::Model],
        db: &C,
    ) -> Result<Self::Output, OrmadaError>;

    /// Populate relations on ModelWithRelations
    fn populate(
        models: &mut [<Parent as crate::traits::WithRelationsTrait>::ModelWithRelations],
        data: &Self::Output,
    );
}

// Base case: no relations to load
impl<E: EntityTrait + crate::traits::WithRelationsTrait> LoadRelations<E> for () {
    type Output = ();

    async fn load_all<C: ConnectionTrait>(
        _models: &[<E as EntityTrait>::Model],
        _db: &C,
    ) -> Result<(), OrmadaError> {
        Ok(())
    }

    fn populate(
        _models: &mut [<E as crate::traits::WithRelationsTrait>::ModelWithRelations],
        _data: &(),
    ) {
    }
}

// JoinLoadRelations: Single relation using JOIN
impl<Parent, R1> JoinLoadRelations<Parent> for RelationSpec<R1>
where
    Parent: EntityTrait
        + HasRelation<R1>
        + crate::traits::WithRelationsTrait<Model = <Parent as EntityTrait>::Model>,
    R1: EntityTrait,
    <Parent as EntityTrait>::Model: Sync,
{
    async fn load_with_join<C: ConnectionTrait>(
        select: Select<Parent>,
        db: &C,
    ) -> Result<Vec<Parent::ModelWithRelations>, OrmadaError> {
        use sea_orm::QuerySelect;

        let relation_def = <Parent as HasRelation<R1>>::relation_def();

        let joined_select = select
            .join(sea_orm::JoinType::LeftJoin, relation_def)
            .select_also(R1::default());

        let results: Vec<(<Parent as EntityTrait>::Model, Option<R1::Model>)> =
            joined_select.all(db).await?;

        let models_with_relations: Vec<Parent::ModelWithRelations> = results
            .into_iter()
            .map(|(model, related)| {
                let mut model_with_rel = Parent::from_model_and_relations(model, &());
                <Parent as HasRelation<R1>>::set_related(&mut model_with_rel, related);
                model_with_rel
            })
            .collect();

        Ok(models_with_relations)
    }

    fn build_join_sql<C: ConnectionTrait>(select: &Select<Parent>, db: &C) -> String {
        use sea_orm::{QuerySelect, QueryTrait};

        let relation_def = <Parent as HasRelation<R1>>::relation_def();

        let joined_select = select
            .clone()
            .join(sea_orm::JoinType::LeftJoin, relation_def)
            .select_also(R1::default());

        let backend = db.get_database_backend();
        let stmt = joined_select.build(backend);
        stmt.to_string()
    }
}

// Single relation (prefetch - separate queries)
impl<Parent, R1> LoadRelations<Parent> for RelationSpec<R1>
where
    Parent: EntityTrait
        + HasRelation<R1>
        + crate::traits::WithRelationsTrait<Model = <Parent as EntityTrait>::Model>,
    R1: EntityTrait,
{
    type Output = FxHashMap<<Parent as HasRelation<R1>>::RelatedPK, R1::Model>;

    async fn load_all<C: ConnectionTrait>(
        models: &[<Parent as EntityTrait>::Model],
        db: &C,
    ) -> Result<Self::Output, OrmadaError> {
        <Parent as HasRelation<R1>>::load_related(models, db).await
    }

    fn populate(
        models: &mut [<Parent as crate::traits::WithRelationsTrait>::ModelWithRelations],
        data: &Self::Output,
    ) {
        for model in models {
            // Use Deref to access the base Model for get_foreign_key
            let pk = <Parent as HasRelation<R1>>::get_foreign_key(&**model);
            let related = data.get(&pk).cloned();
            <Parent as HasRelation<R1>>::set_related(model, related);
        }
    }
}

// Two relations (tuple)
impl<Parent, R1, R2> LoadRelations<Parent> for (RelationSpec<R1>, RelationSpec<R2>)
where
    Parent: EntityTrait
        + HasRelation<R1>
        + HasRelation<R2>
        + crate::traits::WithRelationsTrait<Model = <Parent as EntityTrait>::Model>,
    R1: EntityTrait,
    R2: EntityTrait,
{
    type Output = (
        FxHashMap<<Parent as HasRelation<R1>>::RelatedPK, R1::Model>,
        FxHashMap<<Parent as HasRelation<R2>>::RelatedPK, R2::Model>,
    );

    async fn load_all<C: ConnectionTrait>(
        models: &[<Parent as EntityTrait>::Model],
        db: &C,
    ) -> Result<Self::Output, OrmadaError> {
        // Load relations in parallel for better performance
        let (r1, r2) = futures::join!(
            <Parent as HasRelation<R1>>::load_related(models, db),
            <Parent as HasRelation<R2>>::load_related(models, db)
        );
        Ok((r1?, r2?))
    }

    fn populate(
        models: &mut [<Parent as crate::traits::WithRelationsTrait>::ModelWithRelations],
        data: &Self::Output,
    ) {
        <RelationSpec<R1> as LoadRelations<Parent>>::populate(models, &data.0);
        <RelationSpec<R2> as LoadRelations<Parent>>::populate(models, &data.1);
    }
}

// Note: 3+ tuple implementations removed to reduce complexity
// Users needing 3+ relations can chain multiple queries or implement custom logic
// The single and 2-tuple cases cover 95%+ of real-world use cases

// ============================================================================
// QuerySet with Eager Loading
// ============================================================================

/// `QuerySet` that supports eager loading of relations
pub struct QuerySetEager<'a, E: EntityTrait, C: ConnectionTrait, Relations = ()> {
    pub(crate) db: &'a C,
    pub(crate) select: Select<E>,
    pub(crate) _relations: std::marker::PhantomData<Relations>,
}

impl<'a, E: EntityTrait, C: ConnectionTrait> QuerySetEager<'a, E, C, ()> {
    /// Create from a regular `QuerySet`
    pub const fn new(db: &'a C, select: Select<E>) -> Self {
        Self {
            db,
            select,
            _relations: std::marker::PhantomData,
        }
    }

    /// Add relations to prefetch
    ///
    /// Use the `relations!` macro for clean syntax. You can also chain multiple
    /// calls or pass a raw Vec of `TypeIds`.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use ormada::relations;
    /// use entity::{author::Entity as Author, publisher::Entity as Publisher};
    ///
    /// // Using the relations! macro (recommended)
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
    /// // Access the loaded relations
    /// for book in books {
    ///     println!("Title: {}", book.title);
    ///     if let Some(author) = book.author {
    ///         println!("Author: {}", author.name);
    ///     }
    /// }
    /// ```
    /// Add relations to prefetch (separate queries, 1+M pattern)
    pub fn prefetch_related<R>(self, _relations: R) -> QuerySetEager<'a, E, C, R> {
        QuerySetEager {
            db: self.db,
            select: self.select,
            _relations: std::marker::PhantomData,
        }
    }

    /// Add relations to load via JOIN (single query)
    ///
    /// Uses SQL JOINs to fetch parent and related entities in a single query.
    /// More efficient than `prefetch_related` for many-to-one (FK) and one-to-one relations.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use ormada::relations;
    ///
    /// // Single query with JOIN - same UX as prefetch_related!
    /// let books = Book::objects(db)
    ///     .filter(Book::Published.eq(true))
    ///     .select_related(relations![Author])
    ///     .all()
    ///     .await?;
    ///
    /// // SQL: SELECT books.*, authors.* FROM books LEFT JOIN authors ON ...
    /// for book in books {
    ///     println!("{} by {}", book.title, book.author.name);
    /// }
    /// ```
    ///
    /// # When to Use
    ///
    /// | Relation Type | Recommended Method |
    /// |---------------|-------------------|
    /// | Many-to-One (FK) | `select_related` |
    /// | One-to-One | `select_related` |
    /// | One-to-Many | `prefetch_related` |
    /// | Many-to-Many | `prefetch_related` |
    pub fn select_related<R>(self, _relations: R) -> QuerySetJoined<'a, E, C, R> {
        QuerySetJoined {
            db: self.db,
            select: self.select,
            _relations: std::marker::PhantomData,
        }
    }
}

// Separate impl block for typed relation methods
impl<E, C, Relations> QuerySetEager<'_, E, C, Relations>
where
    E: EntityTrait + crate::traits::WithRelationsTrait<Model = <E as EntityTrait>::Model>,
    <E as EntityTrait>::Model: Sync + Clone,
    <E as crate::traits::WithRelationsTrait>::ModelWithRelations: Clone,
    C: ConnectionTrait,
    Relations: LoadRelations<E>,
{
    /// Get all records with prefetched relations (typed version)
    ///
    /// Returns models with direct field access to relations using compile-time types.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let books: Vec<BookWithRelations> = Book::objects(db)
    ///     .prefetch_related(relations![Author])
    ///     .all()
    ///     .await?;
    ///
    /// for book in books {
    ///     println!("Title: {}", book.title);  // Direct access via Deref
    ///     println!("Author: {}", book.author.name);  // Direct relation access!
    /// }
    /// ```
    pub async fn all(self) -> Result<Vec<E::ModelWithRelations>, OrmadaError> {
        // Execute main query
        let db = self.db;
        let models = self.select.all(db).await?;

        if models.is_empty() {
            return Ok(Vec::new());
        }

        // Load all relations using compile-time typed system
        let relation_data = Relations::load_all(&models, db).await?;

        // Convert Model -> ModelWithRelations, then populate relations
        let mut models_with_relations: Vec<E::ModelWithRelations> = models
            .into_iter()
            .map(|model| E::from_model_and_relations(model, &()))
            .collect();

        // Populate relations on ModelWithRelations
        Relations::populate(&mut models_with_relations, &relation_data);

        Ok(models_with_relations)
    }

    /// Get the first record with prefetched relations
    ///
    /// Returns an error if no records match the query.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use ormada::relations;
    ///
    /// // Get first book with author - DIRECT FIELD ACCESS!
    /// let book: BookWithRelations = Book::objects(db)
    ///     .filter(Column::Published.eq(true))
    ///     .order_by_desc(Column::CreatedAt)
    ///     .prefetch_related(relations![Author])
    ///     .first()
    ///     .await?;
    ///
    /// println!("Latest book: {}", book.title);  // Direct access!
    /// if let Some(author) = book.author {  // Direct relation access!
    ///     println!("Author: {}", author.name);
    /// }
    /// ```
    pub async fn first(self) -> Result<E::ModelWithRelations, OrmadaError> {
        let results = self.all().await?;
        results.into_iter().next().ok_or_else(|| OrmadaError::empty_result_set("first"))
    }

    /// Get the last record with prefetched relations
    ///
    /// Returns an error if no records match the query.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use ormada::relations;
    ///
    /// // Get last book with relations - DIRECT FIELD ACCESS!
    /// let book: BookWithRelations = Book::objects(db)
    ///     .order_by_asc(Column::CreatedAt)
    ///     .prefetch_related(relations![Author, Publisher])
    ///     .last()
    ///     .await?;
    ///
    /// println!("Last book: {}", book.title);  // Direct access!
    /// if let Some(author) = book.author {
    ///     println!("Author: {}", author.name);
    /// }
    /// ```
    pub async fn last(self) -> Result<E::ModelWithRelations, OrmadaError> {
        let results = self.all().await?;
        results.into_iter().last().ok_or_else(|| OrmadaError::empty_result_set("last"))
    }

    /// Count records matching the query
    ///
    /// Returns the number of records that match the query.
    /// This does NOT load relations, just counts the main entities.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Count published books (no relations loaded)
    /// let count = Book::objects(db)
    ///     .filter(Column::Published.eq(true))
    ///     .prefetch_related(relations![Author])  // Ignored for count
    ///     .count()
    ///     .await?;
    /// ```
    pub async fn count(self) -> Result<u64, OrmadaError> {
        let count_select =
            self.select.select_only().column_as(Expr::col(Asterisk).count(), "count");

        let result = count_select.into_tuple::<i64>().one(self.db).await?;
        Ok(result.unwrap_or(0) as u64)
    }

    /// Check if any records exist matching the query
    ///
    /// Returns true if at least one record matches.
    /// This does NOT load relations, just checks existence.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Check if any published books exist
    /// let has_published = Book::objects(db)
    ///     .filter(Column::Published.eq(true))
    ///     .prefetch_related(relations![Author])  // Ignored for exists
    ///     .exists()
    ///     .await?;
    /// ```
    pub async fn exists(self) -> Result<bool, OrmadaError> {
        let result = self.select.limit(1).one(self.db).await?;
        Ok(result.is_some())
    }

    /// Apply limit to the queryset before loading relations
    pub fn limit(mut self, limit: u64) -> Self {
        self.select = self.select.limit(limit);
        self
    }

    /// Apply offset to the queryset before loading relations
    pub fn offset(mut self, offset: u64) -> Self {
        self.select = self.select.offset(offset);
        self
    }

    /// Apply filter to the queryset before loading relations
    pub fn filter(mut self, condition: impl Into<SimpleExpr>) -> Self {
        self.select = self.select.filter(condition.into());
        self
    }

    /// Exclude records matching the condition
    pub fn exclude(mut self, condition: impl Into<Condition>) -> Self {
        let cond: Condition = condition.into();
        self.select = self.select.filter(cond.not());
        self
    }

    /// Order by ascending before loading relations
    pub fn order_by_asc<Col>(mut self, column: Col) -> Self
    where
        Col: ColumnTrait,
    {
        self.select = self.select.order_by_asc(column);
        self
    }

    /// Order by descending before loading relations
    pub fn order_by_desc<Col>(mut self, column: Col) -> Self
    where
        Col: ColumnTrait,
    {
        self.select = self.select.order_by_desc(column);
        self
    }

    /// Apply distinct to the queryset
    pub fn distinct(mut self) -> Self {
        self.select = self.select.distinct();
        self
    }

    /// Get query execution plan
    ///
    /// Executes the EXPLAIN query and returns the database query execution plan.
    /// SQL is pretty-printed by default for readability.
    ///
    /// # Arguments
    /// * `pretty` - Whether to pretty-print the SQL (default: true). Set to false for single-line output.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let plan = Book::objects(&db)
    ///     .prefetch_related(relations![Author])
    ///     .explain(true)
    ///     .await?;
    ///
    /// println!("Query Plan:\n{}", plan);
    /// ```
    pub async fn explain(&self, pretty: bool) -> Result<String, OrmadaError>
    where
        E: crate::traits::OrmadaEntity,
    {
        use sea_orm::QueryTrait;

        let backend = self.db.get_database_backend();
        let stmt = self.select.clone().build(backend);
        let raw_sql = stmt.to_string();
        let explain_sql = crate::format::build_explain_sql(backend, &raw_sql, false);
        let results = self.db.execute_unprepared(&explain_sql).await?;

        Ok(crate::format::format_explain_output(
            &raw_sql,
            pretty,
            results.rows_affected(),
            false,
            None,
        ))
    }

    /// Analyze query with actual execution
    ///
    /// Executes the EXPLAIN ANALYZE query and returns detailed execution statistics.
    /// SQL is pretty-printed by default for readability.
    ///
    /// **⚠️ WARNING**: This actually EXECUTES the query.
    ///
    /// # Arguments
    /// * `pretty` - Whether to pretty-print the SQL (default: true). Set to false for single-line output.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let analysis = Book::objects(&db)
    ///     .prefetch_related(relations![Author])
    ///     .explain_analyze(true)
    ///     .await?;
    ///
    /// println!("Execution Analysis:\n{}", analysis);
    /// ```
    pub async fn explain_analyze(&self, pretty: bool) -> Result<String, OrmadaError>
    where
        E: crate::traits::OrmadaEntity,
    {
        use sea_orm::QueryTrait;

        let backend = self.db.get_database_backend();
        let stmt = self.select.clone().build(backend);
        let raw_sql = stmt.to_string();
        let explain_sql = crate::format::build_explain_sql(backend, &raw_sql, true);
        let results = self.db.execute_unprepared(&explain_sql).await?;

        Ok(crate::format::format_explain_output(
            &raw_sql,
            pretty,
            results.rows_affected(),
            true,
            Some(&explain_sql),
        ))
    }
}

// Note: WithRelations struct removed - replaced by macro-generated ModelWithRelations
// Each entity now has its own ModelWithRelations type with compile-time typed relation fields

// ============================================================================
// QuerySet with JOIN-based Eager Loading (select_related)
// ============================================================================

/// `QuerySet` that uses SQL JOINs for eager loading (Django's `select_related`)
///
/// Unlike `QuerySetEager` which uses separate queries (1+M pattern),
/// `QuerySetJoined` uses SQL JOINs to fetch parent and related entities
/// in a single query. This is more efficient for many-to-one (FK) and one-to-one relations.
///
/// # Design
///
/// - Uses LEFT JOIN to fetch related entities in a single query
/// - Returns `ModelWithRelations` just like `prefetch_related` for unified UX
/// - Single query execution - no additional round trips
///
/// # When to Use
///
/// - **`select_related`**: For FK/1:1 relations (single query with JOIN)
/// - **`prefetch_related`**: For 1:N/M:N relations (separate queries, avoids row duplication)
///
/// # Example
///
/// ```rust,ignore
/// // Single query with JOIN - same UX as prefetch_related!
/// let books = Book::objects(&db)
///     .filter(Book::Published.eq(true))
///     .select_related(relations![Author])
///     .all()
///     .await?;
///
/// for book in books {
///     println!("{} by {}", book.title, book.author.name);
/// }
/// ```
pub struct QuerySetJoined<'a, E: EntityTrait, C: ConnectionTrait, Relations = ()> {
    pub(crate) db: &'a C,
    pub(crate) select: Select<E>,
    pub(crate) _relations: std::marker::PhantomData<Relations>,
}

// Separate impl block for typed relation methods using JoinLoadRelations
impl<E, C, Relations> QuerySetJoined<'_, E, C, Relations>
where
    E: EntityTrait + crate::traits::WithRelationsTrait<Model = <E as EntityTrait>::Model>,
    <E as EntityTrait>::Model: Sync + Clone,
    <E as crate::traits::WithRelationsTrait>::ModelWithRelations: Clone,
    C: ConnectionTrait,
    Relations: JoinLoadRelations<E>,
{
    /// Get all records with joined relations
    ///
    /// Returns `ModelWithRelations` with direct field access to relations.
    /// Uses a single SQL query with JOIN for efficiency.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let books = Book::objects(&db)
    ///     .select_related(relations![Author])
    ///     .all()
    ///     .await?;
    ///
    /// for book in books {
    ///     println!("{} by {}", book.title, book.author.name);
    /// }
    /// ```
    pub async fn all(self) -> Result<Vec<E::ModelWithRelations>, OrmadaError> {
        Relations::load_with_join(self.select, self.db).await
    }

    /// Get the first record with joined relation
    ///
    /// Returns an error if no records match the query.
    pub async fn first(self) -> Result<E::ModelWithRelations, OrmadaError> {
        let limited_select = self.select.limit(1);
        let results = Relations::load_with_join(limited_select, self.db).await?;
        results.into_iter().next().ok_or_else(|| OrmadaError::empty_result_set("first"))
    }

    /// Get the last record with joined relation
    ///
    /// Returns an error if no records match the query.
    pub async fn last(self) -> Result<E::ModelWithRelations, OrmadaError> {
        let results = Relations::load_with_join(self.select, self.db).await?;
        results.into_iter().last().ok_or_else(|| OrmadaError::empty_result_set("last"))
    }

    /// Count records matching the query
    ///
    /// Note: This counts the main entity, not the joined results.
    pub async fn count(self) -> Result<u64, OrmadaError> {
        let count_select =
            self.select.select_only().column_as(Expr::col(Asterisk).count(), "count");

        let result = count_select.into_tuple::<i64>().one(self.db).await?;
        Ok(result.unwrap_or(0) as u64)
    }

    /// Check if any records exist matching the query
    pub async fn exists(self) -> Result<bool, OrmadaError> {
        let result = self.select.limit(1).one(self.db).await?;
        Ok(result.is_some())
    }

    /// Apply limit to the queryset
    pub fn limit(mut self, limit: u64) -> Self {
        self.select = self.select.limit(limit);
        self
    }

    /// Apply offset to the queryset
    pub fn offset(mut self, offset: u64) -> Self {
        self.select = self.select.offset(offset);
        self
    }

    /// Apply filter to the queryset
    pub fn filter<F: Into<SimpleExpr>>(mut self, condition: F) -> Self {
        self.select = self.select.filter(condition.into());
        self
    }

    /// Exclude records matching the condition
    pub fn exclude(mut self, condition: impl Into<Condition>) -> Self {
        let cond: Condition = condition.into();
        self.select = self.select.filter(cond.not());
        self
    }

    /// Order by ascending
    pub fn order_by_asc<Col: ColumnTrait>(mut self, column: Col) -> Self {
        self.select = self.select.order_by_asc(column);
        self
    }

    /// Order by descending
    pub fn order_by_desc<Col: ColumnTrait>(mut self, column: Col) -> Self {
        self.select = self.select.order_by_desc(column);
        self
    }

    /// Apply distinct to the queryset
    pub fn distinct(mut self) -> Self {
        self.select = self.select.distinct();
        self
    }

    /// Get the SQL query for debugging (pretty-printed by default)
    ///
    /// Shows the actual JOIN query that will be executed.
    ///
    /// # Arguments
    /// * `pretty` - Whether to pretty-print the SQL
    pub fn debug_sql(&self, pretty: bool) -> String {
        let sql = Relations::build_join_sql(&self.select, self.db);

        if pretty {
            crate::format::format_sql_pretty(&sql)
        } else {
            sql
        }
    }

    /// Get query execution plan
    ///
    /// Executes the EXPLAIN query and returns the database query execution plan.
    /// Shows the plan for the actual JOIN query.
    ///
    /// # Arguments
    /// * `pretty` - Whether to pretty-print the SQL
    pub async fn explain(&self, pretty: bool) -> Result<String, OrmadaError> {
        let raw_sql = Relations::build_join_sql(&self.select, self.db);
        let backend = self.db.get_database_backend();
        let explain_sql = crate::format::build_explain_sql(backend, &raw_sql, false);
        let results = self.db.execute_unprepared(&explain_sql).await?;

        Ok(crate::format::format_explain_output(
            &raw_sql,
            pretty,
            results.rows_affected(),
            false,
            None,
        ))
    }

    /// Analyze query with actual execution
    ///
    /// **⚠️ WARNING**: This actually EXECUTES the query.
    /// Shows the plan for the actual JOIN query.
    ///
    /// # Arguments
    /// * `pretty` - Whether to pretty-print the SQL
    pub async fn explain_analyze(&self, pretty: bool) -> Result<String, OrmadaError> {
        let raw_sql = Relations::build_join_sql(&self.select, self.db);
        let backend = self.db.get_database_backend();
        let explain_sql = crate::format::build_explain_sql(backend, &raw_sql, true);
        let results = self.db.execute_unprepared(&explain_sql).await?;

        Ok(crate::format::format_explain_output(
            &raw_sql,
            pretty,
            results.rows_affected(),
            true,
            Some(&explain_sql),
        ))
    }
}
