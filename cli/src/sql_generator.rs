//! SQL generation from schema operations

use ormada_schema::{
    ColumnChanges, ColumnSchema, ColumnType, ForeignKeySchema, IndexSchema, OnDeleteAction,
    SchemaOperation, TableSchema,
};
use rustc_hash::{FxHashMap, FxHashSet};

/// Generate SQL for a list of schema operations
/// Orders CREATE TABLE operations by foreign key dependencies
pub fn generate_sql(operations: &[SchemaOperation]) -> String {
    let ordered = order_operations_by_dependencies(operations);
    ordered
        .iter()
        .map(|op| generate_operation_sql(op))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Order operations so that tables are created before tables that reference them
fn order_operations_by_dependencies(operations: &[SchemaOperation]) -> Vec<&SchemaOperation> {
    // Separate CREATE TABLE from other operations
    let mut create_tables: Vec<&TableSchema> = Vec::new();
    let mut other_ops: Vec<&SchemaOperation> = Vec::new();

    for op in operations {
        match op {
            SchemaOperation::CreateTable(schema) => create_tables.push(schema),
            _ => other_ops.push(op),
        }
    }

    if create_tables.is_empty() {
        return operations.iter().collect();
    }

    // Build dependency graph for CREATE TABLE operations
    let table_names: FxHashSet<String> = create_tables.iter().map(|t| t.name.clone()).collect();
    let mut dependencies: FxHashMap<String, Vec<String>> = FxHashMap::default();

    for table in &create_tables {
        let deps: Vec<String> = table
            .foreign_keys
            .iter()
            .filter(|fk| table_names.contains(&fk.references_table))
            .map(|fk| fk.references_table.clone())
            .collect();
        dependencies.insert(table.name.clone(), deps);
    }

    // Topological sort using Kahn's algorithm
    let mut in_degree: FxHashMap<String, usize> = FxHashMap::default();
    for table in &create_tables {
        in_degree.entry(table.name.clone()).or_insert(0);
    }
    // We need to count how many tables depend on each table
    for (table_name, deps) in &dependencies {
        for _dep in deps {
            *in_degree.entry(table_name.clone()).or_insert(0) += 1;
        }
    }

    // Tables with no dependencies (in_degree == 0) come first
    let mut queue: Vec<String> = create_tables
        .iter()
        .filter(|t| dependencies.get(&t.name).is_none_or(|d| d.is_empty()))
        .map(|t| t.name.clone())
        .collect();

    let mut ordered_names: Vec<String> = Vec::new();
    let mut processed: FxHashSet<String> = FxHashSet::default();

    while let Some(name) = queue.pop() {
        if processed.contains(&name) {
            continue;
        }
        ordered_names.push(name.clone());
        processed.insert(name.clone());

        // Find tables that depend on this one and check if all their deps are satisfied
        for table in &create_tables {
            if processed.contains(&table.name) {
                continue;
            }
            if let Some(deps) = dependencies.get(&table.name) {
                if deps.iter().all(|d| processed.contains(d)) {
                    queue.push(table.name.clone());
                }
            }
        }
    }

    // Add any remaining tables (in case of cycles or missing deps)
    for table in &create_tables {
        if !processed.contains(&table.name) {
            ordered_names.push(table.name.clone());
        }
    }

    // Reconstruct operations list with ordered CREATE TABLEs first
    let mut result: Vec<&SchemaOperation> = Vec::new();

    // Add CREATE TABLE operations in dependency order
    for name in &ordered_names {
        if let Some(op) = operations
            .iter()
            .find(|op| matches!(op, SchemaOperation::CreateTable(t) if &t.name == name))
        {
            result.push(op);
        }
    }

    // Add other operations
    result.extend(other_ops);

    result
}

/// Generate SQL for a single schema operation
fn generate_operation_sql(op: &SchemaOperation) -> String {
    match op {
        SchemaOperation::CreateTable(schema) => generate_create_table(schema),
        SchemaOperation::DropTable(name) => format!("DROP TABLE IF EXISTS \"{}\";", name),
        SchemaOperation::RenameTable { from, to } => {
            format!("ALTER TABLE \"{}\" RENAME TO \"{}\";", from, to)
        }
        SchemaOperation::AddColumn { table, column } => {
            format!("ALTER TABLE \"{}\" ADD COLUMN {};", table, generate_column_definition(column))
        }
        SchemaOperation::DropColumn { table, column } => {
            format!("ALTER TABLE \"{}\" DROP COLUMN \"{}\";", table, column)
        }
        SchemaOperation::RenameColumn { table, from, to } => {
            format!("ALTER TABLE \"{}\" RENAME COLUMN \"{}\" TO \"{}\";", table, from, to)
        }
        SchemaOperation::AlterColumn { table, column, changes } => {
            generate_alter_column(table, column, changes)
        }
        SchemaOperation::CreateIndex { table, index } => generate_create_index(table, index),
        SchemaOperation::DropIndex { table: _, name } => {
            format!("DROP INDEX IF EXISTS \"{}\";", name)
        }
        SchemaOperation::AddForeignKey { table, foreign_key } => {
            generate_add_foreign_key(table, foreign_key)
        }
        SchemaOperation::DropForeignKey { table, name } => {
            format!("ALTER TABLE \"{}\" DROP CONSTRAINT \"{}\";", table, name)
        }
    }
}

/// Generate CREATE TABLE SQL
fn generate_create_table(schema: &TableSchema) -> String {
    let mut sql = format!("CREATE TABLE \"{}\" (\n", schema.name);

    let column_defs: Vec<String> = schema
        .columns
        .iter()
        .filter(|c| !c.dropped)
        .map(|c| format!("    {}", generate_column_definition(c)))
        .collect();

    sql.push_str(&column_defs.join(",\n"));

    // Primary key constraint
    if !schema.primary_key.is_empty() {
        sql.push_str(",\n    PRIMARY KEY (");
        sql.push_str(
            &schema
                .primary_key
                .iter()
                .map(|k| format!("\"{}\"", k))
                .collect::<Vec<_>>()
                .join(", "),
        );
        sql.push(')');
    }

    // Foreign key constraints
    for fk in &schema.foreign_keys {
        sql.push_str(",\n    ");
        sql.push_str(&generate_foreign_key_constraint(fk));
    }

    sql.push_str("\n);");

    // Indexes (separate statements)
    for index in &schema.indexes {
        sql.push_str("\n\n");
        sql.push_str(&generate_create_index(&schema.name, index));
    }

    sql
}

/// Generate column definition
fn generate_column_definition(col: &ColumnSchema) -> String {
    let mut def = format!("\"{}\" {}", col.name, column_type_to_sql(&col.column_type));

    if !col.nullable && !col.primary_key {
        def.push_str(" NOT NULL");
    }

    if col.unique {
        def.push_str(" UNIQUE");
    }

    if let Some(ref default) = col.default {
        def.push_str(&format!(" DEFAULT {}", default));
    }

    def
}

/// Convert ColumnType to SQL type
fn column_type_to_sql(col_type: &ColumnType) -> &'static str {
    match col_type {
        ColumnType::Boolean => "BOOLEAN",
        ColumnType::SmallInteger => "SMALLINT",
        ColumnType::Integer => "INTEGER",
        ColumnType::BigInteger => "BIGINT",
        ColumnType::Float => "REAL",
        ColumnType::Double => "DOUBLE PRECISION",
        ColumnType::Decimal { .. } => "DECIMAL",
        ColumnType::String(Some(_)) => "VARCHAR",
        ColumnType::String(None) => "VARCHAR(255)",
        ColumnType::Text => "TEXT",
        ColumnType::Binary => "BYTEA",
        ColumnType::Date => "DATE",
        ColumnType::Time => "TIME",
        ColumnType::DateTime => "TIMESTAMP",
        ColumnType::TimestampTz => "TIMESTAMPTZ",
        ColumnType::Uuid => "UUID",
        ColumnType::Json => "JSON",
        ColumnType::JsonB => "JSONB",
    }
}

/// Generate CREATE INDEX SQL
fn generate_create_index(table: &str, index: &IndexSchema) -> String {
    let unique = if index.unique { "UNIQUE " } else { "" };
    let columns = index
        .columns
        .iter()
        .map(|c| format!("\"{}\"", c))
        .collect::<Vec<_>>()
        .join(", ");

    format!("CREATE {}INDEX \"{}\" ON \"{}\" ({});", unique, index.name, table, columns)
}

/// Generate foreign key constraint
fn generate_foreign_key_constraint(fk: &ForeignKeySchema) -> String {
    let on_delete = match fk.on_delete {
        OnDeleteAction::Cascade => " ON DELETE CASCADE",
        OnDeleteAction::SetNull => " ON DELETE SET NULL",
        OnDeleteAction::SetDefault => " ON DELETE SET DEFAULT",
        OnDeleteAction::Restrict => " ON DELETE RESTRICT",
        OnDeleteAction::NoAction => "",
    };

    format!(
        "CONSTRAINT \"fk_{}_{}\" FOREIGN KEY (\"{}\") REFERENCES \"{}\"(\"{}\"){}",
        fk.column,
        fk.references_table,
        fk.column,
        fk.references_table,
        fk.references_column,
        on_delete
    )
}

/// Generate ADD FOREIGN KEY SQL
fn generate_add_foreign_key(table: &str, fk: &ForeignKeySchema) -> String {
    format!("ALTER TABLE \"{}\" ADD {};", table, generate_foreign_key_constraint(fk))
}

/// Generate ALTER COLUMN SQL for column modifications
fn generate_alter_column(table: &str, column: &str, changes: &ColumnChanges) -> String {
    let mut statements = Vec::new();

    // Type change
    if let Some(ref col_type) = changes.column_type {
        statements.push(format!(
            "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" TYPE {};",
            table,
            column,
            column_type_to_sql(col_type)
        ));
    }

    // Nullable change
    if let Some(nullable) = changes.nullable {
        if nullable {
            statements.push(format!(
                "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" DROP NOT NULL;",
                table, column
            ));
        } else {
            statements.push(format!(
                "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" SET NOT NULL;",
                table, column
            ));
        }
    }

    // Default change - Option<Option<String>> where Some(None) means remove default
    if let Some(ref default_opt) = changes.default {
        match default_opt {
            Some(default_val) => {
                statements.push(format!(
                    "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" SET DEFAULT {};",
                    table, column, default_val
                ));
            }
            None => {
                statements.push(format!(
                    "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" DROP DEFAULT;",
                    table, column
                ));
            }
        }
    }

    if statements.is_empty() {
        format!("-- No changes for column \"{}\" in table \"{}\"", column, table)
    } else {
        statements.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_create_table_simple() {
        let mut schema = TableSchema::new("users");
        let mut id = ColumnSchema::new("id", ColumnType::Integer);
        id.primary_key = true;
        schema.columns.push(id);
        schema.columns.push(ColumnSchema::new("name", ColumnType::String(Some(100))));
        schema.primary_key = vec!["id".to_string()];

        let sql = generate_create_table(&schema);
        assert!(sql.contains("CREATE TABLE \"users\""));
        assert!(sql.contains("\"id\" INTEGER"));
        assert!(sql.contains("\"name\" VARCHAR"));
        assert!(sql.contains("PRIMARY KEY (\"id\")"));
    }

    #[test]
    fn test_generate_create_table_with_foreign_key() {
        let mut schema = TableSchema::new("books");
        schema.columns.push(ColumnSchema::new("id", ColumnType::Integer));
        schema.columns.push(ColumnSchema::new("author_id", ColumnType::Integer));
        schema.foreign_keys.push(
            ForeignKeySchema::new("author_id", "authors", "id").on_delete(OnDeleteAction::Cascade),
        );

        let sql = generate_create_table(&schema);
        assert!(sql.contains("FOREIGN KEY (\"author_id\")"));
        assert!(sql.contains("REFERENCES \"authors\"(\"id\")"));
        assert!(sql.contains("ON DELETE CASCADE"));
    }

    #[test]
    fn test_generate_column_definition_not_null() {
        let col = ColumnSchema::new("email", ColumnType::String(Some(255)));
        let def = generate_column_definition(&col);
        assert!(def.contains("NOT NULL"));
    }

    #[test]
    fn test_generate_column_definition_nullable() {
        let mut col = ColumnSchema::new("bio", ColumnType::Text);
        col.nullable = true;
        let def = generate_column_definition(&col);
        assert!(!def.contains("NOT NULL"));
    }

    #[test]
    fn test_generate_column_definition_with_default() {
        let mut col = ColumnSchema::new("active", ColumnType::Boolean);
        col.default = Some("true".to_string());
        let def = generate_column_definition(&col);
        assert!(def.contains("DEFAULT true"));
    }

    #[test]
    fn test_generate_drop_table() {
        let sql = generate_operation_sql(&SchemaOperation::DropTable("users".to_string()));
        assert_eq!(sql, "DROP TABLE IF EXISTS \"users\";");
    }

    #[test]
    fn test_generate_add_column() {
        let col = ColumnSchema::new("email", ColumnType::String(Some(255)));
        let sql = generate_operation_sql(&SchemaOperation::AddColumn {
            table: "users".to_string(),
            column: col,
        });
        assert!(sql.contains("ALTER TABLE \"users\" ADD COLUMN"));
        assert!(sql.contains("\"email\" VARCHAR"));
    }

    #[test]
    fn test_generate_rename_column() {
        let sql = generate_operation_sql(&SchemaOperation::RenameColumn {
            table: "users".to_string(),
            from: "name".to_string(),
            to: "full_name".to_string(),
        });
        assert!(sql.contains("RENAME COLUMN \"name\" TO \"full_name\""));
    }

    #[test]
    fn test_generate_create_index() {
        let index = IndexSchema {
            name: "idx_users_email".to_string(),
            columns: vec!["email".to_string()],
            unique: true,
        };
        let sql = generate_create_index("users", &index);
        assert!(sql.contains("CREATE UNIQUE INDEX"));
        assert!(sql.contains("\"idx_users_email\""));
        assert!(sql.contains("ON \"users\""));
    }

    #[test]
    fn test_column_type_to_sql() {
        assert_eq!(column_type_to_sql(&ColumnType::Integer), "INTEGER");
        assert_eq!(column_type_to_sql(&ColumnType::TimestampTz), "TIMESTAMPTZ");
        assert_eq!(column_type_to_sql(&ColumnType::Uuid), "UUID");
        assert_eq!(column_type_to_sql(&ColumnType::Text), "TEXT");
    }

    #[test]
    fn test_generate_alter_column_type_change() {
        let changes = ColumnChanges {
            column_type: Some(ColumnType::BigInteger),
            ..Default::default()
        };
        let sql = generate_alter_column("users", "id", &changes);
        assert!(sql.contains("ALTER COLUMN \"id\" TYPE BIGINT"));
    }

    #[test]
    fn test_generate_alter_column_nullable_change() {
        let changes = ColumnChanges {
            nullable: Some(true),
            ..Default::default()
        };
        let sql = generate_alter_column("users", "email", &changes);
        assert!(sql.contains("DROP NOT NULL"));

        let changes2 = ColumnChanges {
            nullable: Some(false),
            ..Default::default()
        };
        let sql2 = generate_alter_column("users", "email", &changes2);
        assert!(sql2.contains("SET NOT NULL"));
    }

    #[test]
    fn test_generate_alter_column_default_change() {
        let changes = ColumnChanges {
            default: Some(Some("true".to_string())),
            ..Default::default()
        };
        let sql = generate_alter_column("users", "active", &changes);
        assert!(sql.contains("SET DEFAULT true"));
    }

    #[test]
    fn test_generate_alter_column_drop_default() {
        let changes = ColumnChanges {
            default: Some(None),
            ..Default::default()
        };
        let sql = generate_alter_column("users", "active", &changes);
        assert!(sql.contains("DROP DEFAULT"));
    }

    #[test]
    fn test_generate_alter_column_no_changes() {
        let changes = ColumnChanges::default();
        let sql = generate_alter_column("users", "id", &changes);
        assert!(sql.contains("-- No changes"));
    }

    #[test]
    fn test_generate_sql_multiple_operations() {
        let ops = vec![
            SchemaOperation::CreateTable(TableSchema::new("users")),
            SchemaOperation::DropTable("old_table".to_string()),
        ];
        let sql = generate_sql(&ops);
        assert!(sql.contains("CREATE TABLE \"users\""));
        assert!(sql.contains("DROP TABLE IF EXISTS \"old_table\""));
    }
}
