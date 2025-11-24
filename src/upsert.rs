//! Bulk upsert operations for efficient INSERT ... ON CONFLICT DO UPDATE

use crate::error::DjangoOrmError;
use sea_orm::{
    sea_query::OnConflict, ActiveModelTrait, ConnectionTrait, EntityTrait, IntoActiveModel,
};

/// Builder for bulk upsert operations
///
/// Provides a fluent API for building INSERT ... ON CONFLICT DO UPDATE queries
pub struct UpsertBuilder<'a, E, C>
where
    E: EntityTrait,
    C: ConnectionTrait,
{
    db: &'a C,
    models: Vec<E::Model>,
    conflict_columns: Option<Vec<E::Column>>,
    update_columns: Option<Vec<E::Column>>,
}

impl<'a, E, C> UpsertBuilder<'a, E, C>
where
    E: EntityTrait,
    C: ConnectionTrait,
{
    /// Create a new `UpsertBuilder`
    pub const fn new(db: &'a C, models: Vec<E::Model>) -> Self {
        Self {
            db,
            models,
            conflict_columns: None,
            update_columns: None,
        }
    }

    /// Set the conflict column (for ON CONFLICT)
    pub fn on_conflict(mut self, column: E::Column) -> Self {
        self.conflict_columns = Some(vec![column]);
        self
    }

    /// Set multiple conflict columns (for composite unique constraints)
    pub fn on_conflict_columns(mut self, columns: Vec<E::Column>) -> Self {
        self.conflict_columns = Some(columns);
        self
    }

    /// Set which fields to update on conflict
    pub fn update_fields(mut self, columns: &[E::Column]) -> Self {
        self.update_columns = Some(columns.to_vec());
        self
    }

    /// Execute the upsert operation
    ///
    /// Returns the number of rows processed
    pub async fn execute(self) -> Result<u64, DjangoOrmError>
    where
        E: crate::traits::DjangoEntity,
        E::Model: IntoActiveModel<E::ActiveModel>,
        E::ActiveModel: ActiveModelTrait<Entity = E> + Send,
    {
        if self.models.is_empty() {
            return Ok(0);
        }

        let conflict_columns = self
            .conflict_columns
            .ok_or_else(|| DjangoOrmError::Custom("on_conflict() must be called".into()))?;

        let update_columns = self
            .update_columns
            .ok_or_else(|| DjangoOrmError::Custom("update_fields() must be called".into()))?;

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
        // For SQLite, use update_columns which should set each column to EXCLUDED.column_name
        let on_conflict =
            OnConflict::columns(conflict_columns).update_columns(update_columns).to_owned();

        query = query.on_conflict(on_conflict);

        query.exec(self.db).await?;

        Ok(count)
    }
}
