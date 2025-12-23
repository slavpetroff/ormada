//! Schema diff algorithm for generating migrations
//!
//! Compares two schema states and generates the operations needed to transform
//! one into the other.

use crate::types::*;

/// Operations that can be performed on a database schema
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaOperation {
    /// Create a new table
    CreateTable(TableSchema),
    /// Drop a table
    DropTable(String),
    /// Rename a table
    RenameTable { from: String, to: String },
    /// Add a column to a table
    AddColumn { table: String, column: ColumnSchema },
    /// Drop a column from a table
    DropColumn { table: String, column: String },
    /// Rename a column
    RenameColumn { table: String, from: String, to: String },
    /// Alter a column (type, nullable, default, etc.)
    AlterColumn {
        table: String,
        column: String,
        changes: ColumnChanges,
    },
    /// Create an index
    CreateIndex { table: String, index: IndexSchema },
    /// Drop an index
    DropIndex { table: String, name: String },
    /// Add a foreign key constraint
    AddForeignKey {
        table: String,
        foreign_key: ForeignKeySchema,
    },
    /// Drop a foreign key constraint
    DropForeignKey { table: String, name: String },
}

/// Changes to apply to a column
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ColumnChanges {
    /// New column type (if changed)
    pub column_type: Option<ColumnType>,
    /// New nullable setting (if changed)
    pub nullable: Option<bool>,
    /// New default value (Some(None) = remove default)
    pub default: Option<Option<String>>,
    /// New max_length (if changed)
    pub max_length: Option<Option<u32>>,
    /// New unique setting (if changed)
    pub unique: Option<bool>,
}

impl ColumnChanges {
    /// Check if there are any changes
    pub fn is_empty(&self) -> bool {
        self.column_type.is_none()
            && self.nullable.is_none()
            && self.default.is_none()
            && self.max_length.is_none()
            && self.unique.is_none()
    }
}

/// Generate diff between two schema states
///
/// Returns a list of operations to transform `current` into `target`.
pub fn generate_diff(current: &[TableSchema], target: &[TableSchema]) -> Vec<SchemaOperation> {
    let mut operations = Vec::new();

    let current_tables: std::collections::HashMap<&str, &TableSchema> =
        current.iter().map(|t| (t.name.as_str(), t)).collect();
    let target_tables: std::collections::HashMap<&str, &TableSchema> =
        target.iter().map(|t| (t.name.as_str(), t)).collect();

    // 1. Find new tables (in target but not in current)
    for (name, schema) in &target_tables {
        if !current_tables.contains_key(name) {
            operations.push(SchemaOperation::CreateTable((*schema).clone()));
        }
    }

    // 2. Find dropped tables (in current but not in target)
    for name in current_tables.keys() {
        if !target_tables.contains_key(name) {
            operations.push(SchemaOperation::DropTable((*name).to_string()));
        }
    }

    // 3. Find table modifications
    for (name, target_table) in &target_tables {
        if let Some(current_table) = current_tables.get(name) {
            operations.extend(diff_table(current_table, target_table));
        }
    }

    // 4. Order operations for safe execution
    order_operations(operations)
}

/// Generate diff for a single table
fn diff_table(current: &TableSchema, target: &TableSchema) -> Vec<SchemaOperation> {
    let mut ops = Vec::new();
    let table = &current.name;

    let current_cols: std::collections::HashMap<&str, &ColumnSchema> =
        current.columns.iter().map(|c| (c.name.as_str(), c)).collect();
    let target_cols: std::collections::HashMap<&str, &ColumnSchema> =
        target.columns.iter().map(|c| (c.name.as_str(), c)).collect();

    // Handle renames first (check renamed_from attribute)
    let mut renamed_from: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    for (name, col) in &target_cols {
        if let Some(ref from) = col.renamed_from {
            renamed_from.insert(from.as_str(), name);
        }
    }

    // Handle drops (check dropped attribute)
    for (name, col) in &target_cols {
        if col.dropped {
            ops.push(SchemaOperation::DropColumn {
                table: table.clone(),
                column: name.to_string(),
            });
        }
    }

    // Process renames
    for (from, to) in &renamed_from {
        if current_cols.contains_key(from) {
            ops.push(SchemaOperation::RenameColumn {
                table: table.clone(),
                from: (*from).to_string(),
                to: (*to).to_string(),
            });
        }
    }

    // New columns (not renamed, not dropped)
    for (name, col) in &target_cols {
        if col.dropped {
            continue;
        }

        // Skip if this is a rename target and source exists
        if col.renamed_from.is_some() {
            continue;
        }

        if !current_cols.contains_key(name) {
            ops.push(SchemaOperation::AddColumn {
                table: table.clone(),
                column: (*col).clone(),
            });
        }
    }

    // Dropped columns (in current but not in target, and not renamed)
    for name in current_cols.keys() {
        if !target_cols.contains_key(name) && !renamed_from.contains_key(name) {
            // Check if any target column has dropped = true for this name
            let explicitly_dropped = target_cols.values().any(|c| c.dropped && c.name == *name);

            if !explicitly_dropped {
                ops.push(SchemaOperation::DropColumn {
                    table: table.clone(),
                    column: (*name).to_string(),
                });
            }
        }
    }

    // Modified columns
    for (name, target_col) in &target_cols {
        if target_col.dropped || target_col.renamed_from.is_some() {
            continue;
        }

        // For renamed columns, compare with the old column
        let current_col = if let Some(ref from) = target_col.renamed_from {
            current_cols.get(from.as_str())
        } else {
            current_cols.get(name)
        };

        if let Some(current_col) = current_col {
            if let Some(changes) = diff_column(current_col, target_col) {
                ops.push(SchemaOperation::AlterColumn {
                    table: table.clone(),
                    column: name.to_string(),
                    changes,
                });
            }
        }
    }

    // Index changes
    ops.extend(diff_indexes(table, &current.indexes, &target.indexes));

    // Foreign key changes
    ops.extend(diff_foreign_keys(table, &current.foreign_keys, &target.foreign_keys));

    ops
}

/// Compare two columns and return changes if different
fn diff_column(current: &ColumnSchema, target: &ColumnSchema) -> Option<ColumnChanges> {
    let mut changes = ColumnChanges::default();

    if current.column_type != target.column_type {
        changes.column_type = Some(target.column_type.clone());
    }

    if current.nullable != target.nullable {
        changes.nullable = Some(target.nullable);
    }

    if current.default != target.default {
        changes.default = Some(target.default.clone());
    }

    if current.max_length != target.max_length {
        changes.max_length = Some(target.max_length);
    }

    if current.unique != target.unique {
        changes.unique = Some(target.unique);
    }

    if changes.is_empty() {
        None
    } else {
        Some(changes)
    }
}

/// Diff indexes between current and target
fn diff_indexes(
    table: &str,
    current: &[IndexSchema],
    target: &[IndexSchema],
) -> Vec<SchemaOperation> {
    let mut ops = Vec::new();

    let current_by_name: std::collections::HashMap<&str, &IndexSchema> =
        current.iter().map(|i| (i.name.as_str(), i)).collect();
    let target_by_name: std::collections::HashMap<&str, &IndexSchema> =
        target.iter().map(|i| (i.name.as_str(), i)).collect();

    // New indexes
    for (name, idx) in &target_by_name {
        if !current_by_name.contains_key(name) {
            ops.push(SchemaOperation::CreateIndex {
                table: table.to_string(),
                index: (*idx).clone(),
            });
        }
    }

    // Dropped indexes
    for name in current_by_name.keys() {
        if !target_by_name.contains_key(name) {
            ops.push(SchemaOperation::DropIndex {
                table: table.to_string(),
                name: (*name).to_string(),
            });
        }
    }

    // Modified indexes (drop and recreate)
    for (name, target_idx) in &target_by_name {
        if let Some(current_idx) = current_by_name.get(name) {
            if current_idx != target_idx {
                ops.push(SchemaOperation::DropIndex {
                    table: table.to_string(),
                    name: (*name).to_string(),
                });
                ops.push(SchemaOperation::CreateIndex {
                    table: table.to_string(),
                    index: (*target_idx).clone(),
                });
            }
        }
    }

    ops
}

/// Diff foreign keys between current and target
fn diff_foreign_keys(
    table: &str,
    current: &[ForeignKeySchema],
    target: &[ForeignKeySchema],
) -> Vec<SchemaOperation> {
    let mut ops = Vec::new();

    // Use column as key since FK names might be auto-generated
    let current_by_col: std::collections::HashMap<&str, &ForeignKeySchema> =
        current.iter().map(|fk| (fk.column.as_str(), fk)).collect();
    let target_by_col: std::collections::HashMap<&str, &ForeignKeySchema> =
        target.iter().map(|fk| (fk.column.as_str(), fk)).collect();

    // New foreign keys
    for (col, fk) in &target_by_col {
        if !current_by_col.contains_key(col) {
            ops.push(SchemaOperation::AddForeignKey {
                table: table.to_string(),
                foreign_key: (*fk).clone(),
            });
        }
    }

    // Dropped foreign keys
    for (col, fk) in &current_by_col {
        if !target_by_col.contains_key(col) {
            let name = fk.name.clone().unwrap_or_else(|| format!("fk_{table}_{col}"));
            ops.push(SchemaOperation::DropForeignKey { table: table.to_string(), name });
        }
    }

    // Modified foreign keys (drop and recreate)
    for (col, target_fk) in &target_by_col {
        if let Some(current_fk) = current_by_col.get(col) {
            if current_fk != target_fk {
                let name = current_fk.name.clone().unwrap_or_else(|| format!("fk_{table}_{col}"));
                ops.push(SchemaOperation::DropForeignKey { table: table.to_string(), name });
                ops.push(SchemaOperation::AddForeignKey {
                    table: table.to_string(),
                    foreign_key: (*target_fk).clone(),
                });
            }
        }
    }

    ops
}

/// Order operations for safe execution
///
/// Order:
/// 1. Drop foreign keys (removes dependencies)
/// 2. Drop indexes
/// 3. Drop columns
/// 4. Drop tables
/// 5. Create tables
/// 6. Add columns
/// 7. Rename columns
/// 8. Alter columns
/// 9. Create indexes
/// 10. Add foreign keys (dependencies must exist)
fn order_operations(mut ops: Vec<SchemaOperation>) -> Vec<SchemaOperation> {
    ops.sort_by_key(|op| match op {
        SchemaOperation::DropForeignKey { .. } => 0,
        SchemaOperation::DropIndex { .. } => 1,
        SchemaOperation::DropColumn { .. } => 2,
        SchemaOperation::DropTable(_) => 3,
        SchemaOperation::CreateTable(_) => 4,
        SchemaOperation::AddColumn { .. } => 5,
        SchemaOperation::RenameColumn { .. } => 6,
        SchemaOperation::RenameTable { .. } => 6,
        SchemaOperation::AlterColumn { .. } => 7,
        SchemaOperation::CreateIndex { .. } => 8,
        SchemaOperation::AddForeignKey { .. } => 9,
    });

    ops
}

/// Resolve a delta migration by merging with base schema
///
/// Takes a base schema and applies delta changes (adds, drops, renames)
/// to produce the final schema state.
pub fn resolve_delta(base: &TableSchema, delta: &TableSchema) -> TableSchema {
    let mut result = base.clone();
    result.migration_id = delta.migration_id.clone();

    // Process each column in delta
    for delta_col in &delta.columns {
        if delta_col.dropped {
            // Remove the column
            result.columns.retain(|c| c.name != delta_col.name);
            continue;
        }

        if let Some(ref from) = delta_col.renamed_from {
            // Rename: find old column and update name
            if let Some(col) = result.columns.iter_mut().find(|c| c.name == *from) {
                col.name = delta_col.name.clone();
                // Also apply any other changes from delta
                if delta_col.max_length.is_some() {
                    col.max_length = delta_col.max_length;
                }
                if delta_col.unique {
                    col.unique = true;
                }
                if delta_col.indexed {
                    col.indexed = true;
                }
            }
            continue;
        }

        // Check if column already exists (modification)
        if let Some(existing) = result.columns.iter_mut().find(|c| c.name == delta_col.name) {
            // Update existing column
            *existing = delta_col.clone();
        } else {
            // Add new column
            result.columns.push(delta_col.clone());
        }
    }

    // Merge indexes
    for idx in &delta.indexes {
        if !result.indexes.iter().any(|i| i.name == idx.name) {
            result.indexes.push(idx.clone());
        }
    }

    // Merge foreign keys
    for fk in &delta.foreign_keys {
        if !result.foreign_keys.iter().any(|f| f.column == fk.column) {
            result.foreign_keys.push(fk.clone());
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_table(name: &str, columns: Vec<ColumnSchema>) -> TableSchema {
        let mut table = TableSchema::new(name);
        for col in columns {
            if col.primary_key {
                table.primary_key.push(col.name.clone());
            }
            table.columns.push(col);
        }
        table
    }

    #[test]
    fn test_diff_new_table() {
        let current: Vec<TableSchema> = vec![];
        let target = vec![make_table(
            "books",
            vec![
                ColumnSchema::new("id", ColumnType::Integer).primary_key(true),
                ColumnSchema::new("title", ColumnType::String(Some(200))),
            ],
        )];

        let ops = generate_diff(&current, &target);

        assert_eq!(ops.len(), 1);
        assert!(matches!(&ops[0], SchemaOperation::CreateTable(t) if t.name == "books"));
    }

    #[test]
    fn test_diff_drop_table() {
        let current = vec![make_table(
            "books",
            vec![ColumnSchema::new("id", ColumnType::Integer).primary_key(true)],
        )];
        let target: Vec<TableSchema> = vec![];

        let ops = generate_diff(&current, &target);

        assert_eq!(ops.len(), 1);
        assert!(matches!(&ops[0], SchemaOperation::DropTable(name) if name == "books"));
    }

    #[test]
    fn test_diff_add_column() {
        let current = vec![make_table(
            "books",
            vec![ColumnSchema::new("id", ColumnType::Integer).primary_key(true)],
        )];
        let target = vec![make_table(
            "books",
            vec![
                ColumnSchema::new("id", ColumnType::Integer).primary_key(true),
                ColumnSchema::new("title", ColumnType::String(Some(200))),
            ],
        )];

        let ops = generate_diff(&current, &target);

        assert_eq!(ops.len(), 1);
        assert!(matches!(
            &ops[0],
            SchemaOperation::AddColumn { table, column }
            if table == "books" && column.name == "title"
        ));
    }

    #[test]
    fn test_diff_drop_column() {
        let current = vec![make_table(
            "books",
            vec![
                ColumnSchema::new("id", ColumnType::Integer).primary_key(true),
                ColumnSchema::new("title", ColumnType::String(Some(200))),
            ],
        )];
        let target = vec![make_table(
            "books",
            vec![ColumnSchema::new("id", ColumnType::Integer).primary_key(true)],
        )];

        let ops = generate_diff(&current, &target);

        assert_eq!(ops.len(), 1);
        assert!(matches!(
            &ops[0],
            SchemaOperation::DropColumn { table, column }
            if table == "books" && column == "title"
        ));
    }

    #[test]
    fn test_diff_rename_column() {
        let current = vec![make_table(
            "books",
            vec![
                ColumnSchema::new("id", ColumnType::Integer).primary_key(true),
                ColumnSchema::new("title", ColumnType::String(Some(200))),
            ],
        )];

        let mut name_col = ColumnSchema::new("name", ColumnType::String(Some(200)));
        name_col.renamed_from = Some("title".to_string());

        let target = vec![make_table(
            "books",
            vec![ColumnSchema::new("id", ColumnType::Integer).primary_key(true), name_col],
        )];

        let ops = generate_diff(&current, &target);

        assert!(ops.iter().any(|op| matches!(
            op,
            SchemaOperation::RenameColumn { table, from, to }
            if table == "books" && from == "title" && to == "name"
        )));
    }

    #[test]
    fn test_resolve_delta_add_column() {
        let base = make_table(
            "books",
            vec![ColumnSchema::new("id", ColumnType::Integer).primary_key(true)],
        );

        let mut delta = TableSchema::new("books");
        delta.migration_id = Some("002".to_string());
        delta.columns.push(ColumnSchema::new("title", ColumnType::String(Some(200))));

        let result = resolve_delta(&base, &delta);

        assert_eq!(result.columns.len(), 2);
        assert!(result.find_column("id").is_some());
        assert!(result.find_column("title").is_some());
        assert_eq!(result.migration_id, Some("002".to_string()));
    }

    #[test]
    fn test_resolve_delta_drop_column() {
        let base = make_table(
            "books",
            vec![
                ColumnSchema::new("id", ColumnType::Integer).primary_key(true),
                ColumnSchema::new("legacy", ColumnType::String(None)),
            ],
        );

        let mut delta = TableSchema::new("books");
        let mut drop_col = ColumnSchema::new("legacy", ColumnType::String(None));
        drop_col.dropped = true;
        delta.columns.push(drop_col);

        let result = resolve_delta(&base, &delta);

        assert_eq!(result.columns.len(), 1);
        assert!(result.find_column("id").is_some());
        assert!(result.find_column("legacy").is_none());
    }

    #[test]
    fn test_operation_ordering() {
        let ops = vec![
            SchemaOperation::AddColumn {
                table: "t".to_string(),
                column: ColumnSchema::new("c", ColumnType::Integer),
            },
            SchemaOperation::DropForeignKey {
                table: "t".to_string(),
                name: "fk".to_string(),
            },
            SchemaOperation::CreateTable(TableSchema::new("new")),
            SchemaOperation::DropTable("old".to_string()),
        ];

        let ordered = order_operations(ops);

        // DropForeignKey should come first
        assert!(matches!(&ordered[0], SchemaOperation::DropForeignKey { .. }));
        // DropTable before CreateTable
        assert!(matches!(&ordered[1], SchemaOperation::DropTable(_)));
        assert!(matches!(&ordered[2], SchemaOperation::CreateTable(_)));
        // AddColumn last
        assert!(matches!(&ordered[3], SchemaOperation::AddColumn { .. }));
    }

    #[test]
    fn test_diff_alter_column_type() {
        let current = vec![make_table(
            "books",
            vec![
                ColumnSchema::new("id", ColumnType::Integer).primary_key(true),
                ColumnSchema::new("price", ColumnType::Integer),
            ],
        )];
        let target = vec![make_table(
            "books",
            vec![
                ColumnSchema::new("id", ColumnType::Integer).primary_key(true),
                ColumnSchema::new("price", ColumnType::Double),
            ],
        )];

        let ops = generate_diff(&current, &target);

        assert_eq!(ops.len(), 1);
        assert!(matches!(
            &ops[0],
            SchemaOperation::AlterColumn { table, column, changes }
            if table == "books" && column == "price" && changes.column_type.is_some()
        ));
    }

    #[test]
    fn test_diff_multiple_tables() {
        let current: Vec<TableSchema> = vec![];
        let target = vec![
            make_table(
                "authors",
                vec![ColumnSchema::new("id", ColumnType::Integer).primary_key(true)],
            ),
            make_table(
                "books",
                vec![ColumnSchema::new("id", ColumnType::Integer).primary_key(true)],
            ),
        ];

        let ops = generate_diff(&current, &target);

        assert_eq!(ops.len(), 2);
        let table_names: Vec<_> = ops
            .iter()
            .filter_map(|op| {
                if let SchemaOperation::CreateTable(t) = op {
                    Some(t.name.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert!(table_names.contains(&"authors"));
        assert!(table_names.contains(&"books"));
    }

    #[test]
    fn test_diff_no_changes() {
        let schema = vec![make_table(
            "books",
            vec![
                ColumnSchema::new("id", ColumnType::Integer).primary_key(true),
                ColumnSchema::new("title", ColumnType::String(Some(200))),
            ],
        )];

        let ops = generate_diff(&schema, &schema);

        assert!(ops.is_empty());
    }

    #[test]
    fn test_resolve_delta_rename_column() {
        let base = make_table(
            "authors",
            vec![
                ColumnSchema::new("id", ColumnType::Integer).primary_key(true),
                ColumnSchema::new("name", ColumnType::String(Some(100))),
            ],
        );

        let mut delta = TableSchema::new("authors");
        let mut renamed_col = ColumnSchema::new("full_name", ColumnType::String(Some(100)));
        renamed_col.renamed_from = Some("name".to_string());
        delta.columns.push(renamed_col);

        let result = resolve_delta(&base, &delta);

        assert_eq!(result.columns.len(), 2);
        assert!(result.find_column("id").is_some());
        assert!(result.find_column("full_name").is_some());
        assert!(result.find_column("name").is_none());
    }

    #[test]
    fn test_resolve_delta_modify_existing_column() {
        let base = make_table(
            "books",
            vec![
                ColumnSchema::new("id", ColumnType::Integer).primary_key(true),
                ColumnSchema::new("title", ColumnType::String(Some(100))),
            ],
        );

        let mut delta = TableSchema::new("books");
        let mut modified_col = ColumnSchema::new("title", ColumnType::String(Some(500)));
        modified_col.unique = true;
        modified_col.max_length = Some(500);
        delta.columns.push(modified_col);

        let result = resolve_delta(&base, &delta);

        assert_eq!(result.columns.len(), 2);
        let title_col = result.find_column("title").unwrap();
        // When column exists, it gets replaced entirely by delta
        assert!(title_col.unique);
        assert_eq!(title_col.column_type, ColumnType::String(Some(500)));
    }

    #[test]
    fn test_diff_add_index() {
        let current = vec![make_table(
            "books",
            vec![ColumnSchema::new("id", ColumnType::Integer).primary_key(true)],
        )];

        let mut target_table = make_table(
            "books",
            vec![ColumnSchema::new("id", ColumnType::Integer).primary_key(true)],
        );
        target_table
            .indexes
            .push(crate::types::IndexSchema::new("idx_books_title", vec!["title".to_string()]));
        let target = vec![target_table];

        let ops = generate_diff(&current, &target);

        assert!(ops.iter().any(|op| matches!(op, SchemaOperation::CreateIndex { .. })));
    }

    #[test]
    fn test_diff_add_foreign_key() {
        let current = vec![make_table(
            "books",
            vec![
                ColumnSchema::new("id", ColumnType::Integer).primary_key(true),
                ColumnSchema::new("author_id", ColumnType::Integer),
            ],
        )];

        let mut target_table = make_table(
            "books",
            vec![
                ColumnSchema::new("id", ColumnType::Integer).primary_key(true),
                ColumnSchema::new("author_id", ColumnType::Integer),
            ],
        );
        target_table.foreign_keys.push(crate::types::ForeignKeySchema::new(
            "author_id",
            "authors",
            "id",
        ));
        let target = vec![target_table];

        let ops = generate_diff(&current, &target);

        assert!(ops.iter().any(|op| matches!(op, SchemaOperation::AddForeignKey { .. })));
    }
}
