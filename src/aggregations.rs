#![allow(clippy::uninlined_format_args)]

//! Aggregation functions for database operations (Ormada's aggregate/annotate)
//!
//! This module provides Ormada-style aggregation functions like COUNT, SUM, AVG, MAX, MIN.
//! These operations are performed at the database level for optimal performance.
//!
//! # Examples
//!
//! ## Basic Aggregations
//!
//! ```rust,ignore
//! use ormada::prelude::*;
//!
//! // Get total count
//! let total = Book::objects(db)
//!     .aggregate_count()
//!     .await?;
//!
//! // Get sum of prices
//! let total_value = Book::objects(db)
//!     .aggregate_sum(Book::Price)
//!     .await?;
//!
//! // Get average price
//! let avg_price = Book::objects(db)
//!     .aggregate_avg(Book::Price)
//!     .await?;
//!
//! // Get max/min
//! let highest_price = Book::objects(db)
//!     .aggregate_max(Book::Price)
//!     .await?;
//!
//! let lowest_price = Book::objects(db)
//!     .aggregate_min(Book::Price)
//!     .await?;
//! ```
//!
//! ## Multiple Aggregations
//!
//! ```rust,ignore
//! // Get multiple aggregate values at once
//! let stats = Book::objects(db)
//!     .filter(Book::Published.eq(true))
//!     .aggregate()
//!     .count()
//!     .sum(Book::Price)
//!     .avg(Book::Price)
//!     .execute()
//!     .await?;
//!
//! println!("Total books: {}", stats.count);
//! println!("Total value: {}", stats.sums.get("price").unwrap());
//! println!("Average price: {}", stats.averages.get("price").unwrap());
//! ```

use crate::error::OrmadaError;
use crate::query::QuerySet;
use rustc_hash::FxHashMap;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, FromQueryResult, QuerySelect};

// ============================================================================
// AggregateValue Enum - Type-safe aggregation results
// ============================================================================

/// Type-safe aggregation result value
///
/// This enum represents the result of an aggregation operation with full type information.
/// Use pattern matching to extract values and handle different aggregation types.
///
/// # Example
///
/// ```rust,ignore
/// use ormada::prelude::*;
///
/// let value = AggregateValue::Sum {
///     column: "price".into(),
///     value: Some(1500.0),
/// };
///
/// match value {
///     AggregateValue::Count(n) => println!("Count: {}", n),
///     AggregateValue::Sum { column, value } => {
///         println!("Sum of {}: {:?}", column, value);
///     }
///     AggregateValue::Avg { column, value } => {
///         println!("Avg of {}: {:?}", column, value);
///     }
///     AggregateValue::Max { column, value } => {
///         println!("Max of {}: {:?}", column, value);
///     }
///     AggregateValue::Min { column, value } => {
///         println!("Min of {}: {:?}", column, value);
///     }
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum AggregateValue {
    /// COUNT result
    Count(u64),
    /// SUM result with column name and optional value (None if no rows)
    Sum {
        /// Column name that was summed
        column: String,
        /// Sum value (None if no matching rows)
        value: Option<f64>,
    },
    /// AVG result with column name and optional value (None if no rows)
    Avg {
        /// Column name that was averaged
        column: String,
        /// Average value (None if no matching rows)
        value: Option<f64>,
    },
    /// MAX result with column name and optional value (None if no rows)
    Max {
        /// Column name for max
        column: String,
        /// Maximum value (None if no matching rows)
        value: Option<f64>,
    },
    /// MIN result with column name and optional value (None if no rows)
    Min {
        /// Column name for min
        column: String,
        /// Minimum value (None if no matching rows)
        value: Option<f64>,
    },
}

impl AggregateValue {
    /// Create a Count result
    pub const fn count(value: u64) -> Self {
        Self::Count(value)
    }

    /// Create a Sum result
    pub fn sum(column: impl Into<String>, value: Option<f64>) -> Self {
        Self::Sum { column: column.into(), value }
    }

    /// Create an Avg result
    pub fn avg(column: impl Into<String>, value: Option<f64>) -> Self {
        Self::Avg { column: column.into(), value }
    }

    /// Create a Max result
    pub fn max(column: impl Into<String>, value: Option<f64>) -> Self {
        Self::Max { column: column.into(), value }
    }

    /// Create a Min result
    pub fn min(column: impl Into<String>, value: Option<f64>) -> Self {
        Self::Min { column: column.into(), value }
    }

    /// Check if this is a Count value
    pub const fn is_count(&self) -> bool {
        matches!(self, Self::Count(_))
    }

    /// Check if this is a Sum value
    pub const fn is_sum(&self) -> bool {
        matches!(self, Self::Sum { .. })
    }

    /// Check if this is an Avg value
    pub const fn is_avg(&self) -> bool {
        matches!(self, Self::Avg { .. })
    }

    /// Check if this is a Max value
    pub const fn is_max(&self) -> bool {
        matches!(self, Self::Max { .. })
    }

    /// Check if this is a Min value
    pub const fn is_min(&self) -> bool {
        matches!(self, Self::Min { .. })
    }

    /// Get the numeric value if present (returns None for Count, use `as_count()` instead)
    #[allow(clippy::cast_precision_loss)]
    pub const fn value(&self) -> Option<f64> {
        match self {
            Self::Count(n) => Some(*n as f64),
            Self::Sum { value, .. }
            | Self::Avg { value, .. }
            | Self::Max { value, .. }
            | Self::Min { value, .. } => *value,
        }
    }

    /// Get the count value if this is a Count
    pub const fn as_count(&self) -> Option<u64> {
        match self {
            Self::Count(n) => Some(*n),
            _ => None,
        }
    }

    /// Get the column name if this is a column-based aggregation
    pub fn column(&self) -> Option<&str> {
        match self {
            Self::Count(_) => None,
            Self::Sum { column, .. }
            | Self::Avg { column, .. }
            | Self::Max { column, .. }
            | Self::Min { column, .. } => Some(column),
        }
    }
}

/// Extension trait for aggregation operations on `QuerySet`
pub trait AggregateExt<E: EntityTrait> {
    /// Count records (Ormada's .`count()`)
    ///
    /// Returns the number of records matching the query.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let count = Book::objects(db)
    ///     .filter(Book::Published.eq(true))
    ///     .aggregate_count()
    ///     .await?;
    ///
    /// println!("Published books: {}", count);
    /// ```
    async fn aggregate_count(self) -> Result<u64, OrmadaError>;

    /// Sum numeric column values (Ormada's .aggregate(Sum('field')))
    ///
    /// Calculates the sum of all values in the specified column.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let total_value = Book::objects(db)
    ///     .aggregate_sum(Book::Price)
    ///     .await?;
    ///
    /// println!("Total inventory value: ${}", total_value.unwrap_or(0.0));
    /// ```
    ///
    /// # Returns
    ///
    /// - `Some(value)` - The sum if records exist
    /// - `None` - If no records match the query
    async fn aggregate_sum(self, column: impl ColumnTrait) -> Result<Option<f64>, OrmadaError>;

    /// Calculate average of numeric column (Ormada's .aggregate(Avg('field')))
    ///
    /// Computes the arithmetic mean of all values in the specified column.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let avg_price = Book::objects(db)
    ///     .filter(Book::Published.eq(true))
    ///     .aggregate_avg(Book::Price)
    ///     .await?;
    ///
    /// println!("Average price: ${:.2}", avg_price.unwrap_or(0.0));
    /// ```
    async fn aggregate_avg(self, column: impl ColumnTrait) -> Result<Option<f64>, OrmadaError>;

    /// Get maximum value (Ormada's .aggregate(Max('field')))
    ///
    /// Finds the maximum value in the specified column.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let max_price = Book::objects(db)
    ///     .aggregate_max(Book::Price)
    ///     .await?;
    ///
    /// println!("Most expensive: ${}", max_price.unwrap_or(0.0));
    /// ```
    async fn aggregate_max(self, column: impl ColumnTrait) -> Result<Option<f64>, OrmadaError>;

    /// Get minimum value (Ormada's .aggregate(Min('field')))
    ///
    /// Finds the minimum value in the specified column.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let min_price = Book::objects(db)
    ///     .filter(Book::Price.gt(0))
    ///     .aggregate_min(Book::Price)
    ///     .await?;
    ///
    /// println!("Cheapest: ${}", min_price.unwrap_or(0.0));
    /// ```
    async fn aggregate_min(self, column: impl ColumnTrait) -> Result<Option<f64>, OrmadaError>;
}

// Helper struct for parsing aggregation results
// SQLite returns INTEGER for integer sums, so we need to handle both
#[derive(Debug, FromQueryResult)]
struct AggregateValueInt {
    value: Option<i64>,
}

#[derive(Debug, FromQueryResult)]
struct AggregateValueFloat {
    value: Option<f64>,
}

impl<
        E: EntityTrait + crate::traits::OrmadaEntity,
        C: ConnectionTrait + Sync,
        S: crate::query::CanExecute,
    > AggregateExt<E> for QuerySet<'_, E, C, S>
where
    E::Model: Send + Sync,
{
    async fn aggregate_count(self) -> Result<u64, OrmadaError> {
        self.count().await
    }

    #[allow(clippy::cast_precision_loss)]
    async fn aggregate_sum(self, column: impl ColumnTrait) -> Result<Option<f64>, OrmadaError> {
        use sea_orm::sea_query::{Expr, Func};
        use sea_orm::DbErr;

        let column_ref = column.as_column_ref();
        let column_expr = Expr::col(column_ref.clone());
        let sum_expr = Func::sum(column_expr);

        let query = self.build_select().select_only().expr_as(sum_expr, "value");

        match query.into_model::<AggregateValueInt>().one(self.inner.db).await {
            Ok(Some(result)) => Ok(result.value.map(|v| v as f64)),
            Ok(None) => Ok(None),
            Err(DbErr::Type(_) | DbErr::Query(_)) => {
                let sum_expr = Func::sum(Expr::col(column_ref));
                let query = self.build_select().select_only().expr_as(sum_expr, "value");

                query
                    .into_model::<AggregateValueFloat>()
                    .one(self.inner.db)
                    .await?
                    .map_or(Ok(None), |result| Ok(result.value))
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn aggregate_avg(self, column: impl ColumnTrait) -> Result<Option<f64>, OrmadaError> {
        use sea_orm::sea_query::{Expr, Func};

        let column_expr = Expr::col(column.as_column_ref());
        let avg_expr = Func::avg(column_expr);

        let query = self.build_select().select_only().expr_as(avg_expr, "value");

        query
            .into_model::<AggregateValueFloat>()
            .one(self.inner.db)
            .await?
            .map_or(Ok(None), |result| Ok(result.value))
    }

    #[allow(clippy::cast_precision_loss)]
    async fn aggregate_max(self, column: impl ColumnTrait) -> Result<Option<f64>, OrmadaError> {
        use sea_orm::sea_query::{Expr, Func};
        use sea_orm::DbErr;

        let column_ref = column.as_column_ref();
        let column_expr = Expr::col(column_ref.clone());
        let max_expr = Func::max(column_expr);

        let query = self.build_select().select_only().expr_as(max_expr, "value");

        match query.into_model::<AggregateValueInt>().one(self.inner.db).await {
            Ok(Some(result)) => Ok(result.value.map(|v| v as f64)),
            Ok(None) => Ok(None),
            Err(DbErr::Type(_) | DbErr::Query(_)) => {
                let max_expr = Func::max(Expr::col(column_ref));
                let query = self.build_select().select_only().expr_as(max_expr, "value");

                query
                    .into_model::<AggregateValueFloat>()
                    .one(self.inner.db)
                    .await?
                    .map_or(Ok(None), |result| Ok(result.value))
            }
            Err(e) => Err(e.into()),
        }
    }

    #[allow(clippy::cast_precision_loss)]
    async fn aggregate_min(self, column: impl ColumnTrait) -> Result<Option<f64>, OrmadaError> {
        use sea_orm::sea_query::{Expr, Func};
        use sea_orm::DbErr;

        let column_ref = column.as_column_ref();
        let column_expr = Expr::col(column_ref.clone());
        let min_expr = Func::min(column_expr);

        let query = self.build_select().select_only().expr_as(min_expr, "value");

        match query.into_model::<AggregateValueInt>().one(self.inner.db).await {
            Ok(Some(result)) => Ok(result.value.map(|v| v as f64)),
            Ok(None) => Ok(None),
            Err(DbErr::Type(_) | DbErr::Query(_)) => {
                let min_expr = Func::min(Expr::col(column_ref));
                let query = self.build_select().select_only().expr_as(min_expr, "value");

                query
                    .into_model::<AggregateValueFloat>()
                    .one(self.inner.db)
                    .await?
                    .map_or(Ok(None), |result| Ok(result.value))
            }
            Err(e) => Err(e.into()),
        }
    }
}

/// Result of multiple aggregations
///
/// Returned when executing multiple aggregate functions at once.
pub struct AggregateResult {
    /// Count of records
    pub count: u64,
    /// Sum values by column name
    pub sums: FxHashMap<String, f64>,
    /// Average values by column name
    pub averages: FxHashMap<String, f64>,
    /// Maximum values by column name
    pub maxes: FxHashMap<String, f64>,
    /// Minimum values by column name
    pub mins: FxHashMap<String, f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aggregate_value_int_some() {
        let result = AggregateValueInt { value: Some(42) };
        assert_eq!(result.value, Some(42));
    }

    #[test]
    fn test_aggregate_value_int_none() {
        let result = AggregateValueInt { value: None };
        assert_eq!(result.value, None);
    }

    #[test]
    fn test_aggregate_value_float_some() {
        let result = AggregateValueFloat { value: Some(42.5) };
        assert_eq!(result.value, Some(42.5));
    }

    #[test]
    fn test_aggregate_value_float_none() {
        let result = AggregateValueFloat { value: None };
        assert_eq!(result.value, None);
    }

    #[test]
    fn test_aggregate_result_construction() {
        let mut sums = FxHashMap::default();
        sums.insert("price".to_string(), 100.0);

        let mut averages = FxHashMap::default();
        averages.insert("price".to_string(), 50.0);

        let mut maxes = FxHashMap::default();
        maxes.insert("price".to_string(), 75.0);

        let mut mins = FxHashMap::default();
        mins.insert("price".to_string(), 25.0);

        let result = AggregateResult { count: 10, sums, averages, maxes, mins };

        assert_eq!(result.count, 10);
        assert_eq!(result.sums.get("price"), Some(&100.0));
        assert_eq!(result.averages.get("price"), Some(&50.0));
        assert_eq!(result.maxes.get("price"), Some(&75.0));
        assert_eq!(result.mins.get("price"), Some(&25.0));
    }

    #[test]
    fn test_aggregate_result_empty_maps() {
        let result = AggregateResult {
            count: 0,
            sums: FxHashMap::default(),
            averages: FxHashMap::default(),
            maxes: FxHashMap::default(),
            mins: FxHashMap::default(),
        };

        assert_eq!(result.count, 0);
        assert!(result.sums.is_empty());
        assert!(result.averages.is_empty());
        assert!(result.maxes.is_empty());
        assert!(result.mins.is_empty());
    }

    // ========================================================================
    // AggregateValue Enum Tests
    // ========================================================================

    #[test]
    fn test_aggregate_value_count() {
        let value = AggregateValue::count(42);
        assert!(value.is_count());
        assert!(!value.is_sum());
        assert_eq!(value.as_count(), Some(42));
        assert_eq!(value.value(), Some(42.0));
        assert_eq!(value.column(), None);
    }

    #[test]
    fn test_aggregate_value_sum() {
        let value = AggregateValue::sum("price", Some(1500.0));
        assert!(value.is_sum());
        assert!(!value.is_count());
        assert_eq!(value.value(), Some(1500.0));
        assert_eq!(value.column(), Some("price"));
    }

    #[test]
    fn test_aggregate_value_avg() {
        let value = AggregateValue::avg("price", Some(50.5));
        assert!(value.is_avg());
        assert_eq!(value.value(), Some(50.5));
        assert_eq!(value.column(), Some("price"));
    }

    #[test]
    fn test_aggregate_value_max() {
        let value = AggregateValue::max("price", Some(999.99));
        assert!(value.is_max());
        assert_eq!(value.value(), Some(999.99));
        assert_eq!(value.column(), Some("price"));
    }

    #[test]
    fn test_aggregate_value_min() {
        let value = AggregateValue::min("price", Some(0.99));
        assert!(value.is_min());
        assert_eq!(value.value(), Some(0.99));
        assert_eq!(value.column(), Some("price"));
    }

    #[test]
    fn test_aggregate_value_none() {
        let value = AggregateValue::sum("price", None);
        assert_eq!(value.value(), None);
        assert_eq!(value.column(), Some("price"));
    }

    #[test]
    fn test_aggregate_value_is_debug() {
        let value = AggregateValue::sum("price", Some(100.0));
        let debug_str = format!("{value:?}");
        assert!(debug_str.contains("Sum"));
        assert!(debug_str.contains("price"));
    }

    #[test]
    fn test_aggregate_value_is_clone() {
        let value = AggregateValue::count(10);
        let cloned = value.clone();
        assert_eq!(value, cloned);
    }

    #[test]
    fn test_aggregate_value_pattern_matching() {
        let values = vec![
            AggregateValue::count(5),
            AggregateValue::sum("a", Some(10.0)),
            AggregateValue::avg("b", Some(2.0)),
            AggregateValue::max("c", Some(100.0)),
            AggregateValue::min("d", Some(1.0)),
        ];

        for value in values {
            match &value {
                AggregateValue::Count(n) => {
                    assert!(value.is_count());
                    assert_eq!(*n, 5);
                }
                AggregateValue::Sum { column, value: v } => {
                    assert!(value.is_sum());
                    assert_eq!(column, "a");
                    assert_eq!(*v, Some(10.0));
                }
                AggregateValue::Avg { column, value: v } => {
                    assert!(value.is_avg());
                    assert_eq!(column, "b");
                    assert_eq!(*v, Some(2.0));
                }
                AggregateValue::Max { column, value: v } => {
                    assert!(value.is_max());
                    assert_eq!(column, "c");
                    assert_eq!(*v, Some(100.0));
                }
                AggregateValue::Min { column, value: v } => {
                    assert!(value.is_min());
                    assert_eq!(column, "d");
                    assert_eq!(*v, Some(1.0));
                }
            }
        }
    }
}
