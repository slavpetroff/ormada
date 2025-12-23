//! Source file parsing for Ormada models and schemas
//!
//! This module parses Rust source files to extract schema information from
//! `#[ormada_model]` and `#[ormada_schema]` attributes.

use std::path::Path;
use syn::{Attribute, Fields, Item, ItemStruct, Type};
use walkdir::WalkDir;

use crate::types::*;

/// Result type for parser operations
pub type ParseResult<T> = Result<T, ParseError>;

/// Errors that can occur during parsing
#[derive(Debug, Clone)]
pub enum ParseError {
    /// Failed to read file
    IoError(String),
    /// Failed to parse Rust syntax
    SyntaxError { file: String, message: String },
    /// Invalid attribute configuration
    InvalidAttribute {
        file: String,
        struct_name: String,
        message: String,
    },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoError(msg) => write!(f, "IO error: {msg}"),
            Self::SyntaxError { file, message } => {
                write!(f, "Syntax error in {file}: {message}")
            }
            Self::InvalidAttribute { file, struct_name, message } => {
                write!(f, "Invalid attribute on {struct_name} in {file}: {message}")
            }
        }
    }
}

impl std::error::Error for ParseError {}

/// Configuration for source discovery
#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    /// Paths to include in scanning
    pub include_paths: Vec<String>,
    /// Paths to exclude from scanning
    pub exclude_paths: Vec<String>,
    /// Skip models with `migrate = false`
    pub skip_non_migratable: bool,
    /// Skip models inside `#[cfg(test)]`
    pub skip_test_models: bool,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            include_paths: vec!["src".to_string()],
            exclude_paths: vec!["tests".to_string(), "examples".to_string(), "benches".to_string()],
            skip_non_migratable: true,
            skip_test_models: true,
        }
    }
}

/// Discover all models from source files
pub fn discover_models(
    project_root: &Path,
    config: &DiscoveryConfig,
) -> ParseResult<Vec<TableSchema>> {
    let mut schemas = Vec::new();

    for include_path in &config.include_paths {
        let search_path = project_root.join(include_path);
        if !search_path.exists() {
            continue;
        }

        for entry in WalkDir::new(&search_path)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
        {
            let path = entry.path();

            // Check exclusions
            let path_str = path.to_string_lossy();
            if config.exclude_paths.iter().any(|ex| path_str.contains(ex)) {
                continue;
            }

            let file_schemas = parse_file(path, config)?;
            schemas.extend(file_schemas);
        }
    }

    Ok(schemas)
}

/// Parse a single Rust file for model definitions
pub fn parse_file(path: &Path, config: &DiscoveryConfig) -> ParseResult<Vec<TableSchema>> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| ParseError::IoError(format!("Failed to read {}: {}", path.display(), e)))?;

    parse_source(&content, path.to_string_lossy().as_ref(), config)
}

/// Parse Rust source code for model definitions
pub fn parse_source(
    source: &str,
    file_name: &str,
    config: &DiscoveryConfig,
) -> ParseResult<Vec<TableSchema>> {
    let file = syn::parse_file(source).map_err(|e| ParseError::SyntaxError {
        file: file_name.to_string(),
        message: e.to_string(),
    })?;

    let mut schemas = Vec::new();

    for item in file.items {
        if let Item::Struct(item_struct) = item {
            // Check for #[ormada_model] or #[ormada_schema]
            if let Some(schema) = parse_ormada_struct(&item_struct, file_name, config)? {
                schemas.push(schema);
            }
        }
    }

    Ok(schemas)
}

/// Parse a struct with #[ormada_model] or #[ormada_schema] attribute
fn parse_ormada_struct(
    item: &ItemStruct,
    file_name: &str,
    config: &DiscoveryConfig,
) -> ParseResult<Option<TableSchema>> {
    // Check for #[cfg(test)] if configured to skip
    if config.skip_test_models && has_cfg_test(&item.attrs) {
        return Ok(None);
    }

    // Look for ormada_model or ormada_schema attribute
    let model_attr = find_ormada_attr(&item.attrs, "ormada_model");
    let schema_attr = find_ormada_attr(&item.attrs, "ormada_schema");

    let attr = match (model_attr, schema_attr) {
        (Some(a), _) => a,
        (_, Some(a)) => a,
        (None, None) => return Ok(None),
    };

    // Parse attribute arguments
    let attr_config = parse_model_attr(attr, file_name, &item.ident.to_string())?;

    // Check migrate flag
    if config.skip_non_migratable && !attr_config.migrate {
        return Ok(None);
    }

    // Parse fields
    let fields = match &item.fields {
        Fields::Named(named) => &named.named,
        _ => {
            return Err(ParseError::InvalidAttribute {
                file: file_name.to_string(),
                struct_name: item.ident.to_string(),
                message: "Only structs with named fields are supported".to_string(),
            });
        }
    };

    let mut table = TableSchema::new(&attr_config.table_name);
    table.migration_id = attr_config.migration_id;

    for field in fields {
        let field_name = field.ident.as_ref().map(|i| i.to_string()).unwrap_or_default();
        let field_type = type_to_string(&field.ty);
        let is_nullable = ColumnType::is_option_type(&field_type);
        let column_type = ColumnType::from_rust_type(&field_type);

        let mut column = ColumnSchema::new(&field_name, column_type);
        column.nullable = is_nullable;

        // Parse field attributes
        parse_field_attrs(&mut column, &mut table, &field.attrs, &field_name)?;

        // Track primary key
        if column.primary_key {
            table.primary_key.push(field_name.clone());
        }

        table.add_column(column);
    }

    Ok(Some(table))
}

/// Parsed model/schema attribute configuration
#[derive(Debug, Default)]
struct ModelAttrConfig {
    table_name: String,
    migration_id: Option<String>,
    after: Option<String>,
    extends: Option<String>,
    migrate: bool,
}

/// Parse #[ormada_model(...)] or #[ormada_schema(...)] attribute
fn parse_model_attr(
    attr: &Attribute,
    file_name: &str,
    struct_name: &str,
) -> ParseResult<ModelAttrConfig> {
    let mut config = ModelAttrConfig { migrate: true, ..Default::default() };

    let meta_list = match &attr.meta {
        syn::Meta::List(list) => list,
        _ => {
            return Err(ParseError::InvalidAttribute {
                file: file_name.to_string(),
                struct_name: struct_name.to_string(),
                message: "Expected attribute with arguments".to_string(),
            });
        }
    };

    // Parse the token stream manually
    let tokens = meta_list.tokens.to_string();

    // Simple key=value parsing
    for part in tokens.split(',') {
        let part = part.trim();
        if let Some((key, value)) = part.split_once('=') {
            let key = key.trim();
            let value = value.trim().trim_matches('"');

            match key {
                "table" => config.table_name = value.to_string(),
                "migration" => config.migration_id = Some(value.to_string()),
                "after" => config.after = Some(value.to_string()),
                "extends" => config.extends = Some(value.to_string()),
                "migrate" => config.migrate = value != "false",
                _ => {}
            }
        }
    }

    if config.table_name.is_empty() {
        return Err(ParseError::InvalidAttribute {
            file: file_name.to_string(),
            struct_name: struct_name.to_string(),
            message: "Missing required 'table' attribute".to_string(),
        });
    }

    Ok(config)
}

/// Parse field attributes and update column/table accordingly
fn parse_field_attrs(
    column: &mut ColumnSchema,
    table: &mut TableSchema,
    attrs: &[Attribute],
    field_name: &str,
) -> ParseResult<()> {
    for attr in attrs {
        let path = attr.path();
        let attr_name = path.get_ident().map(|i| i.to_string()).unwrap_or_default();

        match attr_name.as_str() {
            "primary_key" => {
                column.primary_key = true;
                column.auto_increment = !has_attr_arg(attr, "auto_increment", "false");
            }
            "foreign_key" => {
                if let Some((ref_table, on_delete)) = parse_foreign_key_attr(attr) {
                    let fk =
                        ForeignKeySchema::new(field_name, &ref_table, "id").on_delete(on_delete);
                    table.foreign_keys.push(fk);
                }
            }
            "index" => {
                column.indexed = true;
                column.index_name = get_attr_string_arg(attr, "name");
            }
            "unique" => {
                column.unique = true;
            }
            "max_length" => {
                if let Some(len) = get_attr_int_arg(attr) {
                    column.max_length = Some(len as u32);
                    // Update column type if it's a string
                    if matches!(column.column_type, ColumnType::String(_)) {
                        column.column_type = ColumnType::String(Some(len as u32));
                    }
                }
            }
            "min_length" => {
                if let Some(len) = get_attr_int_arg(attr) {
                    column.min_length = Some(len as u32);
                }
            }
            "range" => {
                column.range = parse_range_attr(attr);
            }
            "default" => {
                column.default = get_attr_value_arg(attr);
            }
            "nullable" => {
                column.nullable = true;
            }
            "soft_delete" => {
                column.soft_delete = true;
            }
            "auto_now" | "auto_now_add" => {
                // These don't affect schema, just runtime behavior
            }
            "rename" => {
                // For delta migrations: #[rename(from = "old_name")]
                // The 'to' is inferred from the field name
                if let Some(from) = get_attr_string_arg(attr, "from") {
                    column.renamed_from = Some(from);
                }
            }
            "drop" => {
                column.dropped = true;
            }
            _ => {}
        }
    }

    Ok(())
}

/// Check if attributes contain #[cfg(test)]
fn has_cfg_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if attr.path().is_ident("cfg") {
            let tokens = attr.meta.to_token_stream().to_string();
            tokens.contains("test")
        } else {
            false
        }
    })
}

/// Find an attribute by name
fn find_ormada_attr<'a>(attrs: &'a [Attribute], name: &str) -> Option<&'a Attribute> {
    attrs.iter().find(|attr| attr.path().is_ident(name))
}

/// Convert syn::Type to string representation
fn type_to_string(ty: &Type) -> String {
    quote::quote!(#ty).to_string().replace(' ', "")
}

/// Check if attribute has a specific argument with a value
fn has_attr_arg(attr: &Attribute, key: &str, value: &str) -> bool {
    let tokens = attr.meta.to_token_stream().to_string();
    tokens.contains(&format!("{key} = {value}")) || tokens.contains(&format!("{key}={value}"))
}

/// Get string argument from attribute
fn get_attr_string_arg(attr: &Attribute, key: &str) -> Option<String> {
    let tokens = attr.meta.to_token_stream().to_string();

    // Look for key = "value" pattern
    for part in tokens.split(',') {
        let part = part.trim();
        if let Some((k, v)) = part.split_once('=') {
            if k.trim() == key {
                return Some(v.trim().trim_matches('"').to_string());
            }
        }
    }
    None
}

/// Get integer argument from attribute (for #[max_length(200)])
fn get_attr_int_arg(attr: &Attribute) -> Option<i64> {
    let tokens = attr.meta.to_token_stream().to_string();

    // Look for pattern like max_length(200)
    if let Some(start) = tokens.find('(') {
        if let Some(end) = tokens.find(')') {
            let inner = &tokens[start + 1..end];
            return inner.trim().parse().ok();
        }
    }
    None
}

/// Get value argument from attribute
fn get_attr_value_arg(attr: &Attribute) -> Option<String> {
    let tokens = attr.meta.to_token_stream().to_string();

    if let Some(start) = tokens.find('(') {
        if let Some(end) = tokens.rfind(')') {
            let inner = &tokens[start + 1..end];
            return Some(inner.trim().trim_matches('"').to_string());
        }
    }
    None
}

/// Parse #[foreign_key(Entity)] or #[foreign_key(Entity, on_delete = Cascade)]
fn parse_foreign_key_attr(attr: &Attribute) -> Option<(String, OnDeleteAction)> {
    let tokens = attr.meta.to_token_stream().to_string();

    // Extract content between parentheses
    let start = tokens.find('(')?;
    let end = tokens.rfind(')')?;
    let inner = &tokens[start + 1..end];

    let parts: Vec<&str> = inner.split(',').collect();

    // First part is the entity path (e.g., "crate::server::models::author::Author")
    let entity_path = parts.first()?.trim();

    // Extract just the entity name from the path (last segment)
    let entity_name = entity_path.rsplit("::").next().unwrap_or(entity_path).trim();

    // Convert entity name to table name (snake_case + 's')
    let table_name = to_table_name(entity_name);

    // Look for on_delete
    let mut on_delete = OnDeleteAction::NoAction;
    for part in parts.iter().skip(1) {
        let part = part.trim();
        if let Some((key, value)) = part.split_once('=') {
            if key.trim() == "on_delete" {
                on_delete = OnDeleteAction::from_str(value.trim());
            }
        }
    }

    Some((table_name, on_delete))
}

/// Parse #[range(min = 0, max = 100)]
fn parse_range_attr(attr: &Attribute) -> Option<RangeConstraint> {
    let tokens = attr.meta.to_token_stream().to_string();

    let mut min = None;
    let mut max = None;

    for part in tokens.split(',') {
        let part = part.trim();
        if let Some((key, value)) = part.split_once('=') {
            let key = key.trim().trim_start_matches("range(").trim_start_matches('(');
            let value = value.trim().trim_end_matches(')');

            match key {
                "min" => min = value.parse().ok(),
                "max" => max = value.parse().ok(),
                _ => {}
            }
        }
    }

    if min.is_some() || max.is_some() {
        Some(RangeConstraint { min, max })
    } else {
        None
    }
}

/// Convert PascalCase entity name to snake_case table name (pluralized)
fn to_table_name(entity: &str) -> String {
    use heck::ToSnakeCase as HeckSnakeCase;
    use inflector::Inflector;

    let snake = HeckSnakeCase::to_snake_case(entity);
    snake.to_plural()
}

use quote::ToTokens;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_model() {
        let source = r#"
            use ormada::prelude::*;
            
            #[ormada_model(table = "books")]
            pub struct Book {
                #[primary_key]
                pub id: i32,
                
                #[max_length(200)]
                pub title: String,
                
                pub published: bool,
            }
        "#;

        let config = DiscoveryConfig::default();
        let schemas = parse_source(source, "test.rs", &config).unwrap();

        assert_eq!(schemas.len(), 1);
        let table = &schemas[0];
        assert_eq!(table.name, "books");
        assert_eq!(table.columns.len(), 3);

        let id_col = table.find_column("id").unwrap();
        assert!(id_col.primary_key);

        let title_col = table.find_column("title").unwrap();
        assert_eq!(title_col.max_length, Some(200));
    }

    #[test]
    fn test_parse_model_with_foreign_key() {
        let source = r#"
            #[ormada_model(table = "books")]
            pub struct Book {
                #[primary_key]
                pub id: i32,
                
                #[foreign_key(Author)]
                pub author_id: i32,
            }
        "#;

        let config = DiscoveryConfig::default();
        let schemas = parse_source(source, "test.rs", &config).unwrap();

        assert_eq!(schemas.len(), 1);
        let table = &schemas[0];
        assert_eq!(table.foreign_keys.len(), 1);

        let fk = &table.foreign_keys[0];
        assert_eq!(fk.column, "author_id");
        assert_eq!(fk.references_table, "authors");
    }

    #[test]
    fn test_parse_schema_with_migration() {
        let source = r#"
            #[ormada_schema(table = "books", migration = "001_initial")]
            pub struct Book {
                #[primary_key]
                pub id: i32,
                pub title: String,
            }
        "#;

        let config = DiscoveryConfig::default();
        let schemas = parse_source(source, "test.rs", &config).unwrap();

        assert_eq!(schemas.len(), 1);
        let table = &schemas[0];
        assert_eq!(table.migration_id, Some("001_initial".to_string()));
    }

    #[test]
    fn test_skip_migrate_false() {
        let source = r#"
            #[ormada_model(table = "test_books", migrate = false)]
            pub struct TestBook {
                #[primary_key]
                pub id: i32,
            }
        "#;

        let config = DiscoveryConfig::default();
        let schemas = parse_source(source, "test.rs", &config).unwrap();

        assert!(schemas.is_empty());
    }

    #[test]
    fn test_parse_foreign_key_with_full_path() {
        let source = r#"
            #[ormada_model(table = "books")]
            pub struct Book {
                #[primary_key]
                pub id: i32,
                
                #[foreign_key(crate::server::models::author::Author, on_delete = Cascade)]
                pub author_id: i32,
            }
        "#;

        let config = DiscoveryConfig::default();
        let schemas = parse_source(source, "test.rs", &config).unwrap();

        assert_eq!(schemas.len(), 1);
        let table = &schemas[0];
        assert_eq!(table.foreign_keys.len(), 1);

        let fk = &table.foreign_keys[0];
        assert_eq!(fk.column, "author_id");
        // Should extract just "Author" from the full path and convert to "authors"
        assert_eq!(fk.references_table, "authors");
        assert_eq!(fk.on_delete, OnDeleteAction::Cascade);
    }

    #[test]
    fn test_parse_foreign_key_simple_name() {
        let source = r#"
            #[ormada_model(table = "books")]
            pub struct Book {
                #[primary_key]
                pub id: i32,
                
                #[foreign_key(Author)]
                pub author_id: i32,
            }
        "#;

        let config = DiscoveryConfig::default();
        let schemas = parse_source(source, "test.rs", &config).unwrap();

        let fk = &schemas[0].foreign_keys[0];
        assert_eq!(fk.references_table, "authors");
        assert_eq!(fk.on_delete, OnDeleteAction::NoAction);
    }

    #[test]
    fn test_parse_datetime_fields() {
        let source = r#"
            #[ormada_model(table = "posts")]
            pub struct Post {
                #[primary_key]
                pub id: i32,
                
                pub created_at: ormada::prelude::DateTimeWithTimeZone,
                pub updated_at: DateTimeWithTimeZone,
                pub published_date: chrono::NaiveDate,
            }
        "#;

        let config = DiscoveryConfig::default();
        let schemas = parse_source(source, "test.rs", &config).unwrap();

        let table = &schemas[0];

        let created_at = table.find_column("created_at").unwrap();
        assert_eq!(created_at.column_type, ColumnType::TimestampTz);

        let updated_at = table.find_column("updated_at").unwrap();
        assert_eq!(updated_at.column_type, ColumnType::TimestampTz);

        let published_date = table.find_column("published_date").unwrap();
        assert_eq!(published_date.column_type, ColumnType::Date);
    }

    #[test]
    fn test_parse_nullable_fields() {
        let source = r#"
            #[ormada_model(table = "users")]
            pub struct User {
                #[primary_key]
                pub id: i32,
                
                pub name: String,
                pub bio: Option<String>,
                pub deleted_at: Option<ormada::prelude::DateTimeWithTimeZone>,
            }
        "#;

        let config = DiscoveryConfig::default();
        let schemas = parse_source(source, "test.rs", &config).unwrap();

        let table = &schemas[0];

        let name = table.find_column("name").unwrap();
        assert!(!name.nullable);

        let bio = table.find_column("bio").unwrap();
        assert!(bio.nullable);
        assert_eq!(bio.column_type, ColumnType::String(None));

        let deleted_at = table.find_column("deleted_at").unwrap();
        assert!(deleted_at.nullable);
        assert_eq!(deleted_at.column_type, ColumnType::TimestampTz);
    }

    #[test]
    fn test_parse_indexed_and_unique_fields() {
        let source = r#"
            #[ormada_model(table = "users")]
            pub struct User {
                #[primary_key]
                pub id: i32,
                
                #[unique]
                pub email: String,
                
                #[index]
                pub username: String,
            }
        "#;

        let config = DiscoveryConfig::default();
        let schemas = parse_source(source, "test.rs", &config).unwrap();

        let table = &schemas[0];

        let email = table.find_column("email").unwrap();
        assert!(email.unique);

        let username = table.find_column("username").unwrap();
        assert!(username.indexed);
    }

    #[test]
    fn test_to_table_name() {
        assert_eq!(to_table_name("Author"), "authors");
        assert_eq!(to_table_name("Book"), "books");
        assert_eq!(to_table_name("Category"), "categories");
        assert_eq!(to_table_name("Address"), "addresses");
    }
}
