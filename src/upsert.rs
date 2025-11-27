//! Bulk upsert operations for efficient INSERT ... ON CONFLICT DO UPDATE
//!
//! Uses typestate pattern to ensure `on_conflict()` and `update_fields()` are called
//! before `execute()` - this is enforced at compile time.

use crate::error::ErgormError;
use sea_orm::{
    sea_query::OnConflict, ActiveModelTrait, ConnectionTrait, EntityTrait, IntoActiveModel,
};
use std::marker::PhantomData;

/// Typestate: needs `on_conflict()` to be called
pub struct NeedsConflict;
/// Typestate: needs `update_fields()` to be called
pub struct NeedsUpdateFields;
/// Typestate: ready to execute
pub struct Ready;

/// Builder for bulk upsert operations with compile-time validation
///
/// Uses typestate pattern to ensure proper configuration before execution.
/// The `execute()` method is only available after both `on_conflict()` and
/// `update_fields()` have been called.
///
/// # Example
///
/// ```rust,ignore
/// Book::objects(db)
///     .upsert_many(books)
///     .on_conflict(Book::Id)           // Returns UpsertBuilder<..., NeedsUpdateFields>
///     .update_fields(&[Book::Title])   // Returns UpsertBuilder<..., Ready>
///     .execute()                        // Only available on Ready state
///     .await?;
/// ```
pub struct UpsertBuilder<'a, E, C, State = NeedsConflict>
where
    E: EntityTrait,
    C: ConnectionTrait,
{
    db: &'a C,
    models: Vec<E::Model>,
    conflict_columns: Vec<E::Column>,
    update_columns: Vec<E::Column>,
    _state: PhantomData<State>,
}

impl<'a, E, C> UpsertBuilder<'a, E, C, NeedsConflict>
where
    E: EntityTrait,
    C: ConnectionTrait,
{
    /// Create a new `UpsertBuilder`
    pub fn new(db: &'a C, models: Vec<E::Model>) -> Self {
        Self {
            db,
            models,
            conflict_columns: Vec::new(),
            update_columns: Vec::new(),
            _state: PhantomData,
        }
    }

    /// Set the conflict column (for ON CONFLICT)
    ///
    /// This must be called before `update_fields()`.
    pub fn on_conflict(self, column: E::Column) -> UpsertBuilder<'a, E, C, NeedsUpdateFields> {
        UpsertBuilder {
            db: self.db,
            models: self.models,
            conflict_columns: vec![column],
            update_columns: self.update_columns,
            _state: PhantomData,
        }
    }

    /// Set multiple conflict columns (for composite unique constraints)
    ///
    /// This must be called before `update_fields()`.
    pub fn on_conflict_columns(
        self,
        columns: Vec<E::Column>,
    ) -> UpsertBuilder<'a, E, C, NeedsUpdateFields> {
        UpsertBuilder {
            db: self.db,
            models: self.models,
            conflict_columns: columns,
            update_columns: self.update_columns,
            _state: PhantomData,
        }
    }
}

impl<'a, E, C> UpsertBuilder<'a, E, C, NeedsUpdateFields>
where
    E: EntityTrait,
    C: ConnectionTrait,
{
    /// Set which fields to update on conflict
    ///
    /// This must be called after `on_conflict()`.
    pub fn update_fields(self, columns: &[E::Column]) -> UpsertBuilder<'a, E, C, Ready> {
        UpsertBuilder {
            db: self.db,
            models: self.models,
            conflict_columns: self.conflict_columns,
            update_columns: columns.to_vec(),
            _state: PhantomData,
        }
    }
}

impl<'a, E, C> UpsertBuilder<'a, E, C, Ready>
where
    E: EntityTrait,
    C: ConnectionTrait,
{
    /// Execute the upsert operation
    ///
    /// Returns the number of rows processed.
    ///
    /// This method is only available after both `on_conflict()` and `update_fields()`
    /// have been called, ensuring compile-time validation of the builder configuration.
    pub async fn execute(self) -> Result<u64, ErgormError>
    where
        E: crate::traits::ErgormEntity,
        E::Model: IntoActiveModel<E::ActiveModel>,
        E::ActiveModel: ActiveModelTrait<Entity = E> + Send,
    {
        if self.models.is_empty() {
            return Ok(0);
        }

        let count = self.models.len() as u64;

        // Convert to ActiveModels using IntoActiveModel (NOT to_active_model_for_create)
        // For upsert, we want all fields Set (including ID) so they can be used in ON CONFLICT
        let active_models: Vec<E::ActiveModel> = self
            .models
            .into_iter()
            .map(sea_orm::IntoActiveModel::into_active_model)
            .collect();

        let mut query = E::insert_many(active_models);

        // Build ON CONFLICT clause
        let on_conflict = OnConflict::columns(self.conflict_columns)
            .update_columns(self.update_columns)
            .to_owned();

        query = query.on_conflict(on_conflict);

        query.exec(self.db).await?;

        Ok(count)
    }
}
