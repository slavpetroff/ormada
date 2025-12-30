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
use std::any::{Any, TypeId};

// ============================================================================
// Reverse Relation Storage
// ============================================================================

/// Storage for reverse relations (one-to-many) loaded via `prefetch_related`
///
/// This provides runtime storage for reverse relations since the parent model
/// doesn't know about child models at compile time (the FK is declared on the child).
///
/// # Example
///
/// ```rust,ignore
/// // Author's ModelWithRelations has this storage
/// let authors = Author::objects(&db)
///     .prefetch_related(reverse_relations![Book])
///     .all()
///     .await?;
///
/// for author in &authors {
///     // Access reverse relations via get_children method
///     let books: &[Book::Model] = author.get_children::<Book>();
/// }
/// ```
#[derive(Debug, Default, Clone)]
pub struct ReverseRelationStorage {
    data: FxHashMap<TypeId, Box<dyn CloneableAny + Send + Sync>>,
}

/// Trait for type-erased cloneable storage
pub trait CloneableAny: Any {
    /// Clone into a boxed trait object
    fn clone_box(&self) -> Box<dyn CloneableAny + Send + Sync>;
    /// Get reference as Any for downcasting
    fn as_any(&self) -> &dyn Any;
    /// Get mutable reference as Any for downcasting
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: Clone + Send + Sync + 'static> CloneableAny for Vec<T> {
    fn clone_box(&self) -> Box<dyn CloneableAny + Send + Sync> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl std::fmt::Debug for dyn CloneableAny + Send + Sync {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CloneableAny")
    }
}

impl Clone for Box<dyn CloneableAny + Send + Sync> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

impl ReverseRelationStorage {
    /// Create a new empty storage
    pub fn new() -> Self {
        Self { data: FxHashMap::default() }
    }

    /// Get children of type T
    ///
    /// Returns an empty slice if no children of this type were loaded.
    pub fn get<T: 'static>(&self) -> &[T] {
        self.data
            .get(&TypeId::of::<Vec<T>>())
            .and_then(|v| v.as_any().downcast_ref::<Vec<T>>())
            .map_or(&[], Vec::as_slice)
    }

    /// Set children of type T
    pub fn set<T: Clone + Send + Sync + 'static>(&mut self, children: Vec<T>) {
        self.data.insert(TypeId::of::<Vec<T>>(), Box::new(children));
    }

    /// Check if children of type T are loaded
    pub fn has<T: 'static>(&self) -> bool {
        self.data.contains_key(&TypeId::of::<Vec<T>>())
    }
}

impl PartialEq for ReverseRelationStorage {
    fn eq(&self, other: &Self) -> bool {
        self.data.len() == other.data.len()
    }
}

impl Eq for ReverseRelationStorage {}

// /// Note: RelationGraph has been removed in favor of compile-time typed relations
// See LoadRelations trait and HasRelation trait for the new zero-cost approach

// ============================================================================
// Relations Macro
// ============================================================================

/// Helper macro to create relation specifications for prefetching
///
/// This macro provides a clean syntax for specifying which relations to prefetch.
/// It works for **both** forward relations (FK) and reverse relations (one-to-many).
///
/// The macro automatically detects the relation type based on context:
/// - **Forward**: When the queried model has a FK to the specified model
/// - **Reverse**: When the specified model has a FK to the queried model
///
/// # Examples
///
/// ```rust,ignore
/// use ormada::relations;
///
/// // Forward relation: Book has FK to Author, load the author
/// let books = Book::objects(db)
///     .prefetch_related(relations![Author])
///     .all()
///     .await?;
///
/// // Reverse relation: Book has FK to Author, load author's books
/// let authors = Author::objects(db)
///     .prefetch_related(relations![Book])
///     .all()
///     .await?;
///
/// // Access loaded relations with get_* methods
/// for author in &authors {
///     let books = author.get_books(&db).await?;
/// }
/// ```
#[macro_export]
macro_rules! relations {
    // Single model - detect forward vs reverse at compile time
    ($model:ty) => {
        $crate::relations::RelationSpec::<< $model as $crate::relations::HasEntityType >::__Entity>::new()
    };
    ($($model:ty),+ $(,)?) => {
        ( $( $crate::relations::RelationSpec::<< $model as $crate::relations::HasEntityType >::__Entity>::new() ),+ )
    };
}

/// Typed relation specification for forward relations (FK)
///
/// Created by the `relations!` macro. For reverse relations, use `reverse_relations!`.
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
///
/// The `Related` type must implement both `EntityTrait` and `WithRelationsTrait`
/// to support nested prefetch (storing `ModelWithRelations` instead of `Model`).
pub trait HasRelation<Related>: EntityTrait + crate::traits::WithRelationsTrait
where
    Related: EntityTrait + crate::traits::WithRelationsTrait,
{
    /// The type of the foreign key (usually i32, but can be other types)
    type RelatedPK: std::cmp::Eq + std::hash::Hash + Clone;

    /// Extract the foreign key from a model
    fn get_foreign_key(model: &<Self as EntityTrait>::Model) -> Self::RelatedPK;

    /// Load related models for a batch of parent models
    /// This must be implemented by the macro since we need access to the specific Column enum
    /// Returns Model (not ModelWithRelations) - conversion happens in set_related
    async fn load_related<C: ConnectionTrait>(
        models: &[<Self as EntityTrait>::Model],
        db: &C,
    ) -> Result<FxHashMap<Self::RelatedPK, <Related as EntityTrait>::Model>, OrmadaError>;

    /// Set the related model on the ModelWithRelations wrapper
    /// This is called during prefetch_related to populate relation fields
    /// Converts Model to ModelWithRelations to enable nested prefetch
    fn set_related(
        model: &mut <Self as crate::traits::WithRelationsTrait>::ModelWithRelations,
        related: Option<<Related as EntityTrait>::Model>,
    );

    /// Set the related ModelWithRelations directly (for nested prefetch)
    /// This allows setting a pre-populated ModelWithRelations with nested relations
    fn set_related_with_relations(
        model: &mut <Self as crate::traits::WithRelationsTrait>::ModelWithRelations,
        related: Option<<Related as crate::traits::WithRelationsTrait>::ModelWithRelations>,
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

/// Trait for reverse relations (one-to-many, e.g., Author → Books)
///
/// This is the inverse of `HasRelation`. While `HasRelation<Author>` on `Book`
/// loads the author for each book, `HasReverseRelation<Book>` on `Author`
/// loads all books for each author.
///
/// # Example
///
/// ```rust,ignore
/// // Author has many Books (reverse of Book.author_id FK)
/// let authors = Author::objects(&db)
///     .prefetch_related(reverse_relations![Book])  // Loads all books per author
///     .all()
///     .await?;
///
/// for author in &authors {
///     let books = author.get_books(&db).await?;  // Unified async interface
///     println!("{} wrote {} books", author.name, books.len());
/// }
/// ```
///
/// This trait is automatically implemented by the `#[ormada_model]` macro
/// when a child model has `#[foreign_key(ParentModel)]`.
pub trait HasReverseRelation<Child: EntityTrait>:
    EntityTrait + crate::traits::WithRelationsTrait
{
    /// The type of the parent's primary key
    type ParentPK: std::cmp::Eq + std::hash::Hash + Clone + Send + Sync;

    /// Extract the primary key from a parent model
    fn get_primary_key(model: &<Self as EntityTrait>::Model) -> Self::ParentPK;

    /// Load child models for a batch of parent models
    ///
    /// Returns a map from parent PK to Vec of child models
    async fn load_children<C: ConnectionTrait>(
        models: &[<Self as EntityTrait>::Model],
        db: &C,
    ) -> Result<FxHashMap<Self::ParentPK, Vec<Child::Model>>, OrmadaError>;

    /// Set the child models on the ModelWithRelations wrapper
    fn set_children(
        model: &mut <Self as crate::traits::WithRelationsTrait>::ModelWithRelations,
        children: Vec<Child::Model>,
    );
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
    R1: EntityTrait + crate::traits::WithRelationsTrait,
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

        let results: Vec<(<Parent as EntityTrait>::Model, Option<<R1 as EntityTrait>::Model>)> =
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
    R1: EntityTrait + crate::traits::WithRelationsTrait,
{
    type Output = FxHashMap<<Parent as HasRelation<R1>>::RelatedPK, <R1 as EntityTrait>::Model>;

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
    R1: EntityTrait + crate::traits::WithRelationsTrait,
    R2: EntityTrait + crate::traits::WithRelationsTrait,
{
    type Output = (
        FxHashMap<<Parent as HasRelation<R1>>::RelatedPK, <R1 as EntityTrait>::Model>,
        FxHashMap<<Parent as HasRelation<R2>>::RelatedPK, <R2 as EntityTrait>::Model>,
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
// Mixed Forward + Reverse Relations (Tuple)
// ============================================================================

// Forward + Reverse relation tuple: (RelationSpec<R>, ReverseRelationSpec<C>)
impl<Parent, R1, C1> LoadRelations<Parent> for (RelationSpec<R1>, ReverseRelationSpec<C1>)
where
    Parent: EntityTrait
        + HasRelation<R1>
        + HasReverseRelation<C1>
        + crate::traits::WithRelationsTrait<Model = <Parent as EntityTrait>::Model>,
    R1: EntityTrait + crate::traits::WithRelationsTrait,
    C1: EntityTrait,
    <Parent as EntityTrait>::Model: Sync,
{
    type Output = (
        FxHashMap<<Parent as HasRelation<R1>>::RelatedPK, <R1 as EntityTrait>::Model>,
        FxHashMap<<Parent as HasReverseRelation<C1>>::ParentPK, Vec<C1::Model>>,
    );

    async fn load_all<C: ConnectionTrait>(
        models: &[<Parent as EntityTrait>::Model],
        db: &C,
    ) -> Result<Self::Output, OrmadaError> {
        let (forward, reverse) = futures::join!(
            <Parent as HasRelation<R1>>::load_related(models, db),
            <Parent as HasReverseRelation<C1>>::load_children(models, db)
        );
        Ok((forward?, reverse?))
    }

    fn populate(
        models: &mut [<Parent as crate::traits::WithRelationsTrait>::ModelWithRelations],
        data: &Self::Output,
    ) {
        // Populate forward relation
        for model in models.iter_mut() {
            let pk = <Parent as HasRelation<R1>>::get_foreign_key(&**model);
            let related = data.0.get(&pk).cloned();
            <Parent as HasRelation<R1>>::set_related(model, related);
        }
        // Populate reverse relation
        for model in models.iter_mut() {
            let pk = <Parent as HasReverseRelation<C1>>::get_primary_key(&**model);
            let children = data.1.get(&pk).cloned().unwrap_or_default();
            <Parent as HasReverseRelation<C1>>::set_children(model, children);
        }
    }
}

// Reverse + Forward relation tuple: (ReverseRelationSpec<C>, RelationSpec<R>)
impl<Parent, C1, R1> LoadRelations<Parent> for (ReverseRelationSpec<C1>, RelationSpec<R1>)
where
    Parent: EntityTrait
        + HasRelation<R1>
        + HasReverseRelation<C1>
        + crate::traits::WithRelationsTrait<Model = <Parent as EntityTrait>::Model>,
    R1: EntityTrait + crate::traits::WithRelationsTrait,
    C1: EntityTrait,
    <Parent as EntityTrait>::Model: Sync,
{
    type Output = (
        FxHashMap<<Parent as HasReverseRelation<C1>>::ParentPK, Vec<C1::Model>>,
        FxHashMap<<Parent as HasRelation<R1>>::RelatedPK, <R1 as EntityTrait>::Model>,
    );

    async fn load_all<C: ConnectionTrait>(
        models: &[<Parent as EntityTrait>::Model],
        db: &C,
    ) -> Result<Self::Output, OrmadaError> {
        let (reverse, forward) = futures::join!(
            <Parent as HasReverseRelation<C1>>::load_children(models, db),
            <Parent as HasRelation<R1>>::load_related(models, db)
        );
        Ok((reverse?, forward?))
    }

    fn populate(
        models: &mut [<Parent as crate::traits::WithRelationsTrait>::ModelWithRelations],
        data: &Self::Output,
    ) {
        // Populate reverse relation
        for model in models.iter_mut() {
            let pk = <Parent as HasReverseRelation<C1>>::get_primary_key(&**model);
            let children = data.0.get(&pk).cloned().unwrap_or_default();
            <Parent as HasReverseRelation<C1>>::set_children(model, children);
        }
        // Populate forward relation
        for model in models.iter_mut() {
            let pk = <Parent as HasRelation<R1>>::get_foreign_key(&**model);
            let related = data.1.get(&pk).cloned();
            <Parent as HasRelation<R1>>::set_related(model, related);
        }
    }
}

// ============================================================================
// Reverse Relation Loading (One-to-Many)
// ============================================================================

/// Marker struct for reverse relations (one-to-many)
///
/// **Note**: The `relations!` macro now handles both forward and reverse relations.
/// This type is kept for backwards compatibility.
pub struct ReverseRelationSpec<E: EntityTrait> {
    _marker: std::marker::PhantomData<E>,
}

impl<E: EntityTrait> ReverseRelationSpec<E> {
    /// Create a new reverse relation specification
    pub const fn new() -> Self {
        Self { _marker: std::marker::PhantomData }
    }
}

impl<E: EntityTrait> Default for ReverseRelationSpec<E> {
    fn default() -> Self {
        Self::new()
    }
}

/// Legacy macro for reverse relations - use `relations!` instead
#[macro_export]
macro_rules! reverse_relations {
    ($model:ty) => {
        $crate::relations::ReverseRelationSpec::<< $model as $crate::relations::HasEntityType >::__Entity>::new()
    };
    ($($model:ty),+ $(,)?) => {
        ( $( $crate::relations::ReverseRelationSpec::<< $model as $crate::relations::HasEntityType >::__Entity>::new() ),+ )
    };
}

// LoadRelations implementation for ReverseRelationSpec (backwards compatibility)
impl<Parent, Child> LoadRelations<Parent> for ReverseRelationSpec<Child>
where
    Parent: EntityTrait
        + HasReverseRelation<Child>
        + crate::traits::WithRelationsTrait<Model = <Parent as EntityTrait>::Model>,
    Child: EntityTrait,
    <Parent as EntityTrait>::Model: Sync,
{
    type Output = FxHashMap<<Parent as HasReverseRelation<Child>>::ParentPK, Vec<Child::Model>>;

    async fn load_all<C: ConnectionTrait>(
        models: &[<Parent as EntityTrait>::Model],
        db: &C,
    ) -> Result<Self::Output, OrmadaError> {
        <Parent as HasReverseRelation<Child>>::load_children(models, db).await
    }

    fn populate(
        models: &mut [<Parent as crate::traits::WithRelationsTrait>::ModelWithRelations],
        data: &Self::Output,
    ) {
        for model in models {
            let pk = <Parent as HasReverseRelation<Child>>::get_primary_key(&**model);
            let children = data.get(&pk).cloned().unwrap_or_default();
            <Parent as HasReverseRelation<Child>>::set_children(model, children);
        }
    }
}

// Two reverse relations (tuple) - backwards compatibility
impl<Parent, C1, C2> LoadRelations<Parent> for (ReverseRelationSpec<C1>, ReverseRelationSpec<C2>)
where
    Parent: EntityTrait
        + HasReverseRelation<C1>
        + HasReverseRelation<C2>
        + crate::traits::WithRelationsTrait<Model = <Parent as EntityTrait>::Model>,
    C1: EntityTrait,
    C2: EntityTrait,
    <Parent as EntityTrait>::Model: Sync,
{
    type Output = (
        FxHashMap<<Parent as HasReverseRelation<C1>>::ParentPK, Vec<C1::Model>>,
        FxHashMap<<Parent as HasReverseRelation<C2>>::ParentPK, Vec<C2::Model>>,
    );

    async fn load_all<C: ConnectionTrait>(
        models: &[<Parent as EntityTrait>::Model],
        db: &C,
    ) -> Result<Self::Output, OrmadaError> {
        let (c1, c2) = futures::join!(
            <Parent as HasReverseRelation<C1>>::load_children(models, db),
            <Parent as HasReverseRelation<C2>>::load_children(models, db)
        );
        Ok((c1?, c2?))
    }

    fn populate(
        models: &mut [<Parent as crate::traits::WithRelationsTrait>::ModelWithRelations],
        data: &Self::Output,
    ) {
        <ReverseRelationSpec<C1> as LoadRelations<Parent>>::populate(models, &data.0);
        <ReverseRelationSpec<C2> as LoadRelations<Parent>>::populate(models, &data.1);
    }
}

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
}

impl<'a, E, C> QuerySetEager<'a, E, C, ()>
where
    E: EntityTrait + crate::traits::WithRelationsTrait<Model = <E as EntityTrait>::Model>,
    <E as EntityTrait>::Model: Sync + Clone,
    C: ConnectionTrait,
{
    /// Add relations to prefetch (chainable)
    ///
    /// Use `with_nested!` for nested prefetch or `reverse_relations!` for reverse relations.
    /// Multiple calls can be chained.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Single nested prefetch
    /// let books = Book::objects(db)
    ///     .prefetch_related(with_nested![Author => Book])
    ///     .all()
    ///     .await?;
    ///
    /// // Chained prefetch for multiple nested relations
    /// let books = Book::objects(db)
    ///     .prefetch_related(with_nested![Author => Book])
    ///     .prefetch_related(with_nested![Publisher => Author])
    ///     .all()
    ///     .await?;
    ///
    /// // Reverse relations
    /// let authors = Author::objects(db)
    ///     .prefetch_related(reverse_relations![Book])
    ///     .all()
    ///     .await?;
    /// ```
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

// Chainable prefetch_related - allows chaining multiple prefetch calls
impl<'a, E, C, R1> QuerySetEager<'a, E, C, R1>
where
    E: EntityTrait + crate::traits::WithRelationsTrait<Model = <E as EntityTrait>::Model>,
    <E as EntityTrait>::Model: Sync + Clone,
    C: ConnectionTrait,
{
    /// Chain another prefetch_related call
    ///
    /// This allows loading multiple nested relations in a single query chain:
    ///
    /// ```rust,ignore
    /// let books = Book::objects(db)
    ///     .prefetch_related(with_nested![Author => Book])
    ///     .prefetch_related(with_nested![Publisher => Author])
    ///     .all()
    ///     .await?;
    /// ```
    pub fn and_prefetch<R2>(self, _relations: R2) -> QuerySetEager<'a, E, C, (R1, R2)> {
        QuerySetEager {
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
