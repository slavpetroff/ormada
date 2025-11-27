//! Type definitions for ormada
//!
//! This module contains type definitions and enums used throughout the library.

/// Defines the behavior when a referenced object is deleted in a foreign key relationship.
///
/// This enum maps to SQL `ON DELETE` clauses and provides type-safe foreign key behavior.
///
/// # Examples
///
/// ```ignore
/// #[ormada_model(table = "posts")]
/// struct Post {
///     #[primary_key]
///     id: i32,
///
///     // Cascade: Delete posts when author is deleted
///     #[foreign_key(Author, on_delete = Cascade)]
///     author_id: i32,
///
///     // SetNull: Set category to NULL when category is deleted
///     #[foreign_key(Category, on_delete = SetNull)]
///     category_id: Option<i32>,  // Must be Option for SetNull
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OnDelete {
    /// Delete related objects when the referenced object is deleted.
    ///
    /// SQL: `ON DELETE CASCADE`
    ///
    /// # Example
    ///
    /// ```ignore
    /// #[foreign_key(Author, on_delete = Cascade)]
    /// author_id: i32,
    /// ```
    ///
    /// When an Author is deleted, all related Posts are automatically deleted.
    Cascade,

    /// Set the foreign key to `NULL` when the referenced object is deleted.
    ///
    /// SQL: `ON DELETE SET NULL`
    ///
    /// **Important**: Field must be `Option<T>` when using `SetNull`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// #[foreign_key(Category, on_delete = SetNull)]
    /// category_id: Option<i32>,  // Must be Option!
    /// ```
    ///
    /// When a Category is deleted, all Posts with that category will have `category_id` set to `NULL`.
    SetNull,

    /// Prevent deletion of the referenced object if any related objects exist.
    ///
    /// SQL: `ON DELETE RESTRICT`
    ///
    /// # Example
    ///
    /// ```ignore
    /// #[foreign_key(Author, on_delete = Restrict)]
    /// author_id: i32,
    /// ```
    ///
    /// Attempting to delete an Author that has Posts will result in a database error.
    Restrict,

    /// Set the foreign key to its default value when the referenced object is deleted.
    ///
    /// SQL: `ON DELETE SET DEFAULT`
    ///
    /// # Example
    ///
    /// ```ignore
    /// #[foreign_key(Status, on_delete = SetDefault, default = 1)]
    /// status_id: i32,
    /// ```
    ///
    /// When a Status is deleted, all related records will have `status_id` set to `1`.
    SetDefault,

    /// Let the database handle the deletion behavior.
    ///
    /// SQL: `ON DELETE NO ACTION`
    ///
    /// This is similar to `Restrict` but the check is deferred until the end of the transaction.
    ///
    /// # Example
    ///
    /// ```ignore
    /// #[foreign_key(Author, on_delete = NoAction)]
    /// author_id: i32,
    /// ```
    NoAction,
}

impl OnDelete {
    /// Convert the enum variant to its SQL string representation.
    ///
    /// This is used during migration generation to create the appropriate SQL.
    ///
    /// # Examples
    ///
    /// ```
    /// use ormada::types::OnDelete;
    ///
    /// assert_eq!(OnDelete::Cascade.to_sql(), "CASCADE");
    /// assert_eq!(OnDelete::SetNull.to_sql(), "SET NULL");
    /// assert_eq!(OnDelete::Restrict.to_sql(), "RESTRICT");
    /// ```
    #[inline]
    pub const fn to_sql(&self) -> &'static str {
        match self {
            Self::Cascade => "CASCADE",
            Self::SetNull => "SET NULL",
            Self::Restrict => "RESTRICT",
            Self::SetDefault => "SET DEFAULT",
            Self::NoAction => "NO ACTION",
        }
    }

    /// Check if this `OnDelete` variant requires the field to be nullable.
    ///
    /// Returns `true` for `SetNull`, `false` for all others.
    ///
    /// # Examples
    ///
    /// ```
    /// use ormada::types::OnDelete;
    ///
    /// assert!(OnDelete::SetNull.requires_nullable());
    /// assert!(!OnDelete::Cascade.requires_nullable());
    /// assert!(!OnDelete::Restrict.requires_nullable());
    /// ```
    #[inline]
    pub const fn requires_nullable(&self) -> bool {
        matches!(self, Self::SetNull)
    }
}

impl std::fmt::Display for OnDelete {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_sql())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_on_delete_to_sql() {
        assert_eq!(OnDelete::Cascade.to_sql(), "CASCADE");
        assert_eq!(OnDelete::SetNull.to_sql(), "SET NULL");
        assert_eq!(OnDelete::Restrict.to_sql(), "RESTRICT");
        assert_eq!(OnDelete::SetDefault.to_sql(), "SET DEFAULT");
        assert_eq!(OnDelete::NoAction.to_sql(), "NO ACTION");
    }

    #[test]
    fn test_requires_nullable() {
        assert!(OnDelete::SetNull.requires_nullable());
        assert!(!OnDelete::Cascade.requires_nullable());
        assert!(!OnDelete::Restrict.requires_nullable());
        assert!(!OnDelete::SetDefault.requires_nullable());
        assert!(!OnDelete::NoAction.requires_nullable());
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", OnDelete::Cascade), "CASCADE");
        assert_eq!(format!("{}", OnDelete::SetNull), "SET NULL");
    }

    #[test]
    fn test_enum_properties() {
        // Test that enum is Copy and Clone
        let on_delete = OnDelete::Cascade;
        let copy = on_delete;
        let clone = on_delete;
        assert_eq!(copy, clone);

        // Test equality
        assert_eq!(OnDelete::Cascade, OnDelete::Cascade);
        assert_ne!(OnDelete::Cascade, OnDelete::SetNull);
    }
}
