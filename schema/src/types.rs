//! Schema type definitions for Ormada migrations
//!
//! These types represent database schema in a database-agnostic way.
//! They are used for:
//! - Representing parsed `#[ormada_model]` and `#[ormada_schema]` definitions
//! - Comparing schemas to generate migration diffs
//! - Serializing schema state for migration files

use serde::{Deserialize, Serialize};

/// Represents a complete database table schema
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableSchema {
    /// Table name in the database
    pub name: String,
    /// Columns in the table
    pub columns: Vec<ColumnSchema>,
    /// Indexes on the table (excluding primary key)
    pub indexes: Vec<IndexSchema>,
    /// Foreign key constraints
    pub foreign_keys: Vec<ForeignKeySchema>,
    /// Primary key columns (supports composite keys)
    pub primary_key: Vec<String>,
    /// Migration ID this schema belongs to (for tracking)
    pub migration_id: Option<String>,
}

impl TableSchema {
    /// Create a new empty table schema
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            columns: Vec::new(),
            indexes: Vec::new(),
            foreign_keys: Vec::new(),
            primary_key: Vec::new(),
            migration_id: None,
        }
    }

    /// Add a column to the schema
    pub fn add_column(&mut self, column: ColumnSchema) {
        self.columns.push(column);
    }

    /// Find a column by name
    pub fn find_column(&self, name: &str) -> Option<&ColumnSchema> {
        self.columns.iter().find(|c| c.name == name)
    }

    /// Find a column by name (mutable)
    pub fn find_column_mut(&mut self, name: &str) -> Option<&mut ColumnSchema> {
        self.columns.iter_mut().find(|c| c.name == name)
    }
}

/// Represents a database column
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnSchema {
    /// Column name
    pub name: String,
    /// Column data type
    pub column_type: ColumnType,
    /// Whether the column allows NULL values
    pub nullable: bool,
    /// Default value expression (SQL string)
    pub default: Option<String>,
    /// Whether the column has a unique constraint
    pub unique: bool,
    /// Whether this column is part of the primary key
    pub primary_key: bool,
    /// Whether the primary key auto-increments
    pub auto_increment: bool,
    /// Whether this column has an index
    pub indexed: bool,
    /// Index name if indexed
    pub index_name: Option<String>,
    /// Max length for string types
    pub max_length: Option<u32>,
    /// Min length for string types (validation only)
    pub min_length: Option<u32>,
    /// Range constraints for numeric types (validation only)
    pub range: Option<RangeConstraint>,
    /// Whether this is a soft delete marker column
    pub soft_delete: bool,
    /// For delta migrations: this column was renamed from another
    pub renamed_from: Option<String>,
    /// For delta migrations: this column should be dropped
    pub dropped: bool,
}

impl ColumnSchema {
    /// Create a new column with the given name and type
    pub fn new(name: impl Into<String>, column_type: ColumnType) -> Self {
        Self {
            name: name.into(),
            column_type,
            nullable: false,
            default: None,
            unique: false,
            primary_key: false,
            auto_increment: false,
            indexed: false,
            index_name: None,
            max_length: None,
            min_length: None,
            range: None,
            soft_delete: false,
            renamed_from: None,
            dropped: false,
        }
    }

    /// Set nullable
    pub fn nullable(mut self, nullable: bool) -> Self {
        self.nullable = nullable;
        self
    }

    /// Set default value
    pub fn default(mut self, default: impl Into<String>) -> Self {
        self.default = Some(default.into());
        self
    }

    /// Set as primary key
    pub fn primary_key(mut self, auto_increment: bool) -> Self {
        self.primary_key = true;
        self.auto_increment = auto_increment;
        self
    }

    /// Set as indexed
    pub fn indexed(mut self) -> Self {
        self.indexed = true;
        self
    }

    /// Set as unique
    pub fn unique(mut self) -> Self {
        self.unique = true;
        self
    }

    /// Set max length
    pub fn max_length(mut self, len: u32) -> Self {
        self.max_length = Some(len);
        self
    }
}

/// Database column types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnType {
    /// Boolean type
    Boolean,
    /// Small integer (i16)
    SmallInteger,
    /// Integer (i32)
    Integer,
    /// Big integer (i64)
    BigInteger,
    /// Single precision float (f32)
    Float,
    /// Double precision float (f64)
    Double,
    /// Decimal with precision and scale
    Decimal { precision: u32, scale: u32 },
    /// Variable-length string with optional max length
    String(Option<u32>),
    /// Unlimited text
    Text,
    /// Binary data
    Binary,
    /// Date (no time)
    Date,
    /// Time (no date)
    Time,
    /// DateTime without timezone
    DateTime,
    /// DateTime with timezone
    TimestampTz,
    /// UUID
    Uuid,
    /// JSON
    Json,
    /// JSONB (PostgreSQL)
    JsonB,
}

impl ColumnType {
    /// Infer column type from Rust type string
    pub fn from_rust_type(type_str: &str) -> Self {
        let type_str = type_str.trim();

        // Handle Option<T>
        if type_str.starts_with("Option<") && type_str.ends_with('>') {
            let inner = &type_str[7..type_str.len() - 1];
            return Self::from_rust_type(inner);
        }

        // Handle paths like ormada::prelude::DateTimeWithTimeZone
        let type_str = if let Some(last) = type_str.rsplit("::").next() { last } else { type_str };

        match type_str {
            "bool" => Self::Boolean,
            "i16" => Self::SmallInteger,
            "i32" => Self::Integer,
            "i64" => Self::BigInteger,
            "f32" => Self::Float,
            "f64" => Self::Double,
            "String" => Self::String(None),
            "Vec<u8>" => Self::Binary,
            "Uuid" => Self::Uuid,
            "NaiveDate" | "Date" => Self::Date,
            "NaiveTime" | "Time" => Self::Time,
            "NaiveDateTime" | "DateTime" => Self::DateTime,
            "DateTimeWithTimeZone" | "DateTime<FixedOffset>" => Self::TimestampTz,
            "Value" => Self::Json,
            _ => Self::String(None), // Default fallback
        }
    }

    /// Check if this type is nullable by default (Option types)
    pub fn is_option_type(type_str: &str) -> bool {
        type_str.trim().starts_with("Option<")
    }
}

/// Range constraint for numeric columns
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RangeConstraint {
    pub min: Option<i64>,
    pub max: Option<i64>,
}

/// Index definition
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexSchema {
    /// Index name
    pub name: String,
    /// Columns in the index
    pub columns: Vec<String>,
    /// Whether this is a unique index
    pub unique: bool,
}

impl IndexSchema {
    /// Create a new index
    pub fn new(name: impl Into<String>, columns: Vec<String>) -> Self {
        Self {
            name: name.into(),
            columns,
            unique: false,
        }
    }

    /// Set as unique index
    pub fn unique(mut self) -> Self {
        self.unique = true;
        self
    }
}

/// Foreign key constraint
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForeignKeySchema {
    /// Constraint name
    pub name: Option<String>,
    /// Column in this table
    pub column: String,
    /// Referenced table
    pub references_table: String,
    /// Referenced column
    pub references_column: String,
    /// ON DELETE behavior
    pub on_delete: OnDeleteAction,
    /// ON UPDATE behavior
    pub on_update: OnUpdateAction,
}

impl ForeignKeySchema {
    /// Create a new foreign key
    pub fn new(
        column: impl Into<String>,
        references_table: impl Into<String>,
        references_column: impl Into<String>,
    ) -> Self {
        Self {
            name: None,
            column: column.into(),
            references_table: references_table.into(),
            references_column: references_column.into(),
            on_delete: OnDeleteAction::NoAction,
            on_update: OnUpdateAction::NoAction,
        }
    }

    /// Set ON DELETE action
    pub fn on_delete(mut self, action: OnDeleteAction) -> Self {
        self.on_delete = action;
        self
    }
}

/// ON DELETE actions for foreign keys
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum OnDeleteAction {
    #[default]
    NoAction,
    Restrict,
    Cascade,
    SetNull,
    SetDefault,
}

impl OnDeleteAction {
    /// Parse from string (case-insensitive)
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "cascade" => Self::Cascade,
            "restrict" => Self::Restrict,
            "setnull" | "set_null" | "set null" => Self::SetNull,
            "setdefault" | "set_default" | "set default" => Self::SetDefault,
            _ => Self::NoAction,
        }
    }
}

/// ON UPDATE actions for foreign keys
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum OnUpdateAction {
    #[default]
    NoAction,
    Restrict,
    Cascade,
    SetNull,
    SetDefault,
}

/// Migration metadata
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationMeta {
    /// Migration ID (matches filename without extension)
    pub id: String,
    /// Migration this one depends on (for ordering)
    pub after: Option<String>,
    /// Tables defined or modified in this migration
    pub tables: Vec<TableSchema>,
    /// Data migration function name (if any)
    pub data_migration: Option<String>,
}

impl MigrationMeta {
    /// Create a new migration
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            after: None,
            tables: Vec::new(),
            data_migration: None,
        }
    }

    /// Set the dependency
    pub fn after(mut self, after: impl Into<String>) -> Self {
        self.after = Some(after.into());
        self
    }

    /// Add a table schema
    pub fn add_table(&mut self, table: TableSchema) {
        self.tables.push(table);
    }
}

/// Schema delta for a table (used in extends migrations)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableDelta {
    /// Table name
    pub table: String,
    /// Base migration this extends
    pub extends: String,
    /// Columns to add
    pub add_columns: Vec<ColumnSchema>,
    /// Columns to drop (by name)
    pub drop_columns: Vec<String>,
    /// Column renames (from -> to)
    pub rename_columns: Vec<(String, String)>,
    /// Column modifications
    pub alter_columns: Vec<ColumnSchema>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_column_type_from_rust_type_primitives() {
        assert_eq!(ColumnType::from_rust_type("bool"), ColumnType::Boolean);
        assert_eq!(ColumnType::from_rust_type("i16"), ColumnType::SmallInteger);
        assert_eq!(ColumnType::from_rust_type("i32"), ColumnType::Integer);
        assert_eq!(ColumnType::from_rust_type("i64"), ColumnType::BigInteger);
        assert_eq!(ColumnType::from_rust_type("f32"), ColumnType::Float);
        assert_eq!(ColumnType::from_rust_type("f64"), ColumnType::Double);
        assert_eq!(ColumnType::from_rust_type("String"), ColumnType::String(None));
    }

    #[test]
    fn test_column_type_from_rust_type_datetime() {
        // Simple names
        assert_eq!(ColumnType::from_rust_type("DateTimeWithTimeZone"), ColumnType::TimestampTz);
        assert_eq!(ColumnType::from_rust_type("NaiveDateTime"), ColumnType::DateTime);
        assert_eq!(ColumnType::from_rust_type("NaiveDate"), ColumnType::Date);
        assert_eq!(ColumnType::from_rust_type("NaiveTime"), ColumnType::Time);

        // Full paths (as they appear when parsed from source)
        assert_eq!(
            ColumnType::from_rust_type("ormada::prelude::DateTimeWithTimeZone"),
            ColumnType::TimestampTz
        );
        assert_eq!(ColumnType::from_rust_type("chrono::NaiveDateTime"), ColumnType::DateTime);
        assert_eq!(ColumnType::from_rust_type("chrono::NaiveDate"), ColumnType::Date);
        assert_eq!(ColumnType::from_rust_type("chrono::NaiveTime"), ColumnType::Time);
    }

    #[test]
    fn test_column_type_from_rust_type_special() {
        assert_eq!(ColumnType::from_rust_type("Uuid"), ColumnType::Uuid);
        assert_eq!(ColumnType::from_rust_type("uuid::Uuid"), ColumnType::Uuid);
        assert_eq!(ColumnType::from_rust_type("Vec<u8>"), ColumnType::Binary);
        assert_eq!(ColumnType::from_rust_type("Value"), ColumnType::Json);
        assert_eq!(ColumnType::from_rust_type("serde_json::Value"), ColumnType::Json);
    }

    #[test]
    fn test_column_type_from_rust_type_option() {
        // Option wrapping should unwrap and parse inner type
        assert_eq!(ColumnType::from_rust_type("Option<i32>"), ColumnType::Integer);
        assert_eq!(ColumnType::from_rust_type("Option<String>"), ColumnType::String(None));
        assert_eq!(
            ColumnType::from_rust_type("Option<DateTimeWithTimeZone>"),
            ColumnType::TimestampTz
        );
        assert_eq!(
            ColumnType::from_rust_type("Option<ormada::prelude::DateTimeWithTimeZone>"),
            ColumnType::TimestampTz
        );
    }

    #[test]
    fn test_column_type_from_rust_type_unknown_fallback() {
        // Unknown types should fall back to String
        assert_eq!(ColumnType::from_rust_type("CustomType"), ColumnType::String(None));
        assert_eq!(ColumnType::from_rust_type("my_module::MyType"), ColumnType::String(None));
    }

    #[test]
    fn test_is_option_type() {
        assert!(ColumnType::is_option_type("Option<i32>"));
        assert!(ColumnType::is_option_type("Option<String>"));
        assert!(ColumnType::is_option_type("Option<DateTimeWithTimeZone>"));
        assert!(!ColumnType::is_option_type("i32"));
        assert!(!ColumnType::is_option_type("String"));
        assert!(!ColumnType::is_option_type("DateTimeWithTimeZone"));
    }

    #[test]
    fn test_table_schema_builder() {
        let mut table = TableSchema::new("books");
        table.add_column(ColumnSchema::new("id", ColumnType::Integer).primary_key(true));
        table.add_column(ColumnSchema::new("title", ColumnType::String(Some(200))).max_length(200));
        table.primary_key = vec!["id".to_string()];

        assert_eq!(table.name, "books");
        assert_eq!(table.columns.len(), 2);
        assert!(table.find_column("id").is_some());
        assert!(table.find_column("title").is_some());
        assert!(table.find_column("nonexistent").is_none());
    }

    #[test]
    fn test_on_delete_from_str() {
        assert_eq!(OnDeleteAction::from_str("Cascade"), OnDeleteAction::Cascade);
        assert_eq!(OnDeleteAction::from_str("CASCADE"), OnDeleteAction::Cascade);
        assert_eq!(OnDeleteAction::from_str("SetNull"), OnDeleteAction::SetNull);
        assert_eq!(OnDeleteAction::from_str("set_null"), OnDeleteAction::SetNull);
        assert_eq!(OnDeleteAction::from_str("unknown"), OnDeleteAction::NoAction);
    }
}
