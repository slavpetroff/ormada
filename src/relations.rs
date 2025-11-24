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
//! This is achieved via the `#[derive(DjangoModel)]` macro which reads SeaORM's
//! Relation definitions and generates the extended struct at compile time.

use crate::error::DjangoOrmError;
use rustc_hash::FxHashMap;
use sea_orm::{ConnectionTrait, EntityTrait, Select};

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
/// use seaorm_django::relations;
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
pub struct RelationSpec<E: sea_orm::EntityTrait> {
    _marker: std::marker::PhantomData<E>,
}

impl<E: sea_orm::EntityTrait> RelationSpec<E> {
    /// Create a new relation specification (zero-cost, compile-time only)
    pub const fn new() -> Self {
        Self { _marker: std::marker::PhantomData }
    }
}

impl<E: sea_orm::EntityTrait> Default for RelationSpec<E> {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Relation Loading Traits
// ============================================================================

/// Trait to extract Entity type from Model (for relations! macro)
///
/// This is automatically implemented by the #[django_model] macro
#[doc(hidden)]
pub trait HasEntityType {
    type __Entity: sea_orm::EntityTrait;
}

/// Trait for entities that can load a specific relation
pub trait HasRelation<Related: EntityTrait>: EntityTrait {
    /// The type of the foreign key (usually i32, but can be other types)
    type RelatedPK: std::cmp::Eq + std::hash::Hash + Clone;

    /// Extract the foreign key from a model
    fn get_foreign_key(model: &Self::Model) -> Self::RelatedPK;

    /// Load related models for a batch of parent models
    /// This must be implemented by the macro since we need access to the specific Column enum
    async fn load_related<C: ConnectionTrait>(
        models: &[Self::Model],
        db: &C,
    ) -> Result<FxHashMap<Self::RelatedPK, Related::Model>, DjangoOrmError>;

    /// Set the related model on the parent model
    fn set_related(model: &mut Self::Model, related: Option<Related::Model>);
}

/// Trait for loading relations at compile time
pub trait LoadRelations<Parent: EntityTrait> {
    /// The type of data loaded (tuple of HashMaps)
    type Output;

    /// Load all relations
    async fn load_all<C: ConnectionTrait>(
        models: &[Parent::Model],
        db: &C,
    ) -> Result<Self::Output, DjangoOrmError>;

    /// Populate relations on models
    fn populate(models: &mut [Parent::Model], data: &Self::Output);
}

// Base case: no relations to load
impl<E: EntityTrait> LoadRelations<E> for () {
    type Output = ();

    async fn load_all<C: ConnectionTrait>(
        _models: &[E::Model],
        _db: &C,
    ) -> Result<(), DjangoOrmError> {
        Ok(())
    }

    fn populate(_models: &mut [E::Model], _data: &()) {}
}

// Single relation
impl<Parent, R1> LoadRelations<Parent> for RelationSpec<R1>
where
    Parent: EntityTrait + HasRelation<R1>,
    R1: EntityTrait,
{
    type Output = FxHashMap<<Parent as HasRelation<R1>>::RelatedPK, R1::Model>;

    async fn load_all<C: ConnectionTrait>(
        models: &[Parent::Model],
        db: &C,
    ) -> Result<Self::Output, DjangoOrmError> {
        <Parent as HasRelation<R1>>::load_related(models, db).await
    }

    fn populate(models: &mut [Parent::Model], data: &Self::Output) {
        for model in models {
            let pk = <Parent as HasRelation<R1>>::get_foreign_key(model);
            let related = data.get(&pk).cloned();
            <Parent as HasRelation<R1>>::set_related(model, related);
        }
    }
}

// Two relations (tuple)
impl<Parent, R1, R2> LoadRelations<Parent> for (RelationSpec<R1>, RelationSpec<R2>)
where
    Parent: EntityTrait + HasRelation<R1> + HasRelation<R2>,
    R1: EntityTrait,
    R2: EntityTrait,
{
    type Output = (
        FxHashMap<<Parent as HasRelation<R1>>::RelatedPK, R1::Model>,
        FxHashMap<<Parent as HasRelation<R2>>::RelatedPK, R2::Model>,
    );

    async fn load_all<C: ConnectionTrait>(
        models: &[Parent::Model],
        db: &C,
    ) -> Result<Self::Output, DjangoOrmError> {
        let r1 = <Parent as HasRelation<R1>>::load_related(models, db).await?;
        let r2 = <Parent as HasRelation<R2>>::load_related(models, db).await?;
        Ok((r1, r2))
    }

    fn populate(models: &mut [Parent::Model], data: &Self::Output) {
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

/// QuerySet that supports eager loading of relations
pub struct QuerySetEager<'a, E: EntityTrait, C: ConnectionTrait, Relations = ()> {
    pub(crate) db: &'a C,
    pub(crate) select: Select<E>,
    pub(crate) _relations: std::marker::PhantomData<Relations>,
}

impl<'a, E: EntityTrait, C: ConnectionTrait> QuerySetEager<'a, E, C, ()> {
    /// Create from a regular QuerySet
    pub fn new(db: &'a C, select: Select<E>) -> Self {
        Self {
            db,
            select,
            _relations: std::marker::PhantomData,
        }
    }

    /// Add relations to prefetch
    ///
    /// Use the `relations!` macro for clean syntax. You can also chain multiple
    /// calls or pass a raw Vec of TypeIds.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use seaorm_django::relations;
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
    /// Add relations to prefetch - typed version
    pub fn prefetch_related<R>(self, _relations: R) -> QuerySetEager<'a, E, C, R> {
        QuerySetEager {
            db: self.db,
            select: self.select,
            _relations: std::marker::PhantomData,
        }
    }
}

// Separate impl block for typed relation methods
impl<'a, E, C, Relations> QuerySetEager<'a, E, C, Relations>
where
    E: EntityTrait + crate::traits::WithRelationsTrait<Model = <E as EntityTrait>::Model>,
    <E as EntityTrait>::Model: Sync + Clone,
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
    ///     println!("Title: {}", book.title);  // Direct access
    ///     if let Some(author) = book.author {  // Direct relation access!
    ///         println!("Author: {}", author.name);
    ///     }
    /// }
    /// ```
    pub async fn all(self) -> Result<Vec<E::ModelWithRelations>, DjangoOrmError> {
        // Execute main query
        let db = self.db;
        let mut models = self.select.all(db).await?;

        if models.is_empty() {
            return Ok(Vec::new());
        }

        // Load all relations using compile-time typed system
        let relation_data = Relations::load_all(&models, db).await?;

        // Populate relations on models
        Relations::populate(&mut models, &relation_data);

        // Build results (identity transform now)
        let results = models
            .into_iter()
            .map(|model| E::from_model_and_relations(model, &()))
            .collect();

        Ok(results)
    }

    /// Get the first record with prefetched relations
    ///
    /// Returns an error if no records match the query.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use seaorm_django::relations;
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
    pub async fn first(self) -> Result<E::ModelWithRelations, DjangoOrmError> {
        let results = self.all().await?;
        results
            .into_iter()
            .next()
            .ok_or_else(|| DjangoOrmError::Custom("No records found".into()))
    }

    /// Get the last record with prefetched relations
    ///
    /// Returns an error if no records match the query.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use seaorm_django::relations;
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
    pub async fn last(self) -> Result<E::ModelWithRelations, DjangoOrmError> {
        let results = self.all().await?;
        results
            .into_iter()
            .last()
            .ok_or_else(|| DjangoOrmError::Custom("No records found".into()))
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
    pub async fn count(self) -> Result<u64, DjangoOrmError> {
        use sea_orm::QuerySelect;
        let count_select = self.select.select_only().column_as(
            sea_orm::sea_query::Expr::col(sea_orm::sea_query::Asterisk).count(),
            "count",
        );

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
    pub async fn exists(self) -> Result<bool, DjangoOrmError> {
        use sea_orm::QuerySelect;
        let result = self.select.limit(1).one(self.db).await?;
        Ok(result.is_some())
    }

    /// Apply limit to the queryset before loading relations
    pub fn limit(mut self, limit: u64) -> Self {
        use sea_orm::QuerySelect;
        self.select = self.select.limit(limit);
        self
    }

    /// Apply offset to the queryset before loading relations
    pub fn offset(mut self, offset: u64) -> Self {
        use sea_orm::QuerySelect;
        self.select = self.select.offset(offset);
        self
    }

    /// Apply filter to the queryset before loading relations
    pub fn filter(mut self, condition: impl Into<sea_orm::sea_query::SimpleExpr>) -> Self {
        use sea_orm::QueryFilter;
        self.select = self.select.filter(condition.into());
        self
    }

    /// Exclude records matching the condition
    pub fn exclude(mut self, condition: impl Into<sea_orm::Condition>) -> Self {
        use sea_orm::QueryFilter;
        let cond: sea_orm::Condition = condition.into();
        self.select = self.select.filter(cond.not());
        self
    }

    /// Order by ascending before loading relations
    pub fn order_by_asc<Col>(mut self, column: Col) -> Self
    where
        Col: sea_orm::ColumnTrait,
    {
        use sea_orm::QueryOrder;
        self.select = self.select.order_by_asc(column);
        self
    }

    /// Order by descending before loading relations
    pub fn order_by_desc<Col>(mut self, column: Col) -> Self
    where
        Col: sea_orm::ColumnTrait,
    {
        use sea_orm::QueryOrder;
        self.select = self.select.order_by_desc(column);
        self
    }

    /// Apply distinct to the queryset
    pub fn distinct(mut self) -> Self {
        use sea_orm::QuerySelect;
        self.select = self.select.distinct();
        self
    }

    // Note: build_relation_graph removed - replaced by compile-time LoadRelations trait
}

// Note: WithRelations struct removed - replaced by macro-generated ModelWithRelations
// Each entity now has its own ModelWithRelations type with compile-time typed relation fields
