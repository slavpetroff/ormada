//! Implementation of the `#[ormada_schema]` attribute macro for migrations
//!
//! This macro is used in migration files to define schema snapshots.
//! Unlike `#[ormada_model]`, it doesn't generate runtime code - it's purely
//! declarative and parsed by the CLI to generate migrations.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    Attribute, Data, DeriveInput, Fields, Ident, Token,
};

/// Configuration for the `#[ormada_schema]` attribute
#[derive(Debug, Clone, Default)]
pub struct SchemaConfig {
    /// Table name in the database
    pub table_name: String,
    /// Migration ID this schema belongs to
    pub migration_id: Option<String>,
    /// Migration this one depends on (for ordering)
    pub after: Option<String>,
    /// Entity to extend (for delta migrations)
    pub extends: Option<String>,
    /// Whether to include in migrations (default: true)
    pub migrate: bool,
}

impl Parse for SchemaConfig {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut config = SchemaConfig { migrate: true, ..Default::default() };

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            match key.to_string().as_str() {
                "table" => {
                    let value: syn::LitStr = input.parse()?;
                    config.table_name = value.value();
                }
                "migration" => {
                    let value: syn::LitStr = input.parse()?;
                    config.migration_id = Some(value.value());
                }
                "after" => {
                    let value: syn::LitStr = input.parse()?;
                    config.after = Some(value.value());
                }
                "extends" => {
                    let value: Ident = input.parse()?;
                    config.extends = Some(value.to_string());
                }
                "migrate" => {
                    let value: syn::LitBool = input.parse()?;
                    config.migrate = value.value();
                }
                _ => {
                    return Err(syn::Error::new(key.span(), format!("Unknown attribute: {key}")));
                }
            }

            // Optional comma
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        if config.table_name.is_empty() {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "Missing required 'table' attribute",
            ));
        }

        Ok(config)
    }
}

/// Main implementation of the `#[ormada_schema]` attribute macro
pub fn impl_ormada_schema(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let config: SchemaConfig = syn::parse2(attr)?;
    let mut input: DeriveInput = syn::parse2(item)?;

    let struct_name = &input.ident;
    let vis = &input.vis;

    // Extract and process fields - strip our custom attributes
    let fields = match &mut input.data {
        Data::Struct(data) => match &mut data.fields {
            Fields::Named(fields) => fields,
            _ => {
                return Err(syn::Error::new_spanned(
                    struct_name,
                    "ormada_schema only supports structs with named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                struct_name,
                "ormada_schema only supports structs",
            ));
        }
    };

    // Strip ormada-specific attributes from fields (they're for CLI parsing, not compilation)
    let ormada_attrs = [
        "primary_key",
        "foreign_key",
        "one_to_one",
        "many_to_many",
        "index",
        "unique",
        "max_length",
        "min_length",
        "range",
        "auto_now",
        "auto_now_add",
        "soft_delete",
        "default",
        "nullable",
        "skip_serializing",
        "skip_deserializing",
        "rename",
        "drop",
    ];

    for field in fields.named.iter_mut() {
        field.attrs.retain(|attr| {
            let path = attr.path();
            !ormada_attrs.iter().any(|name| path.is_ident(name))
        });
    }

    let field_defs: Vec<_> = fields.named.iter().collect();

    let table_name = &config.table_name;
    let migration_id = config.migration_id.as_deref().unwrap_or("");
    let after = config.after.as_deref().unwrap_or("");
    let extends = config.extends.as_deref().unwrap_or("");

    // Generate a marker trait implementation for schema discovery
    // This allows the CLI to identify schema structs at compile time
    let expanded = quote! {
        #[derive(Clone, Debug)]
        #vis struct #struct_name {
            #(#field_defs,)*
        }

        // Metadata for CLI discovery (compile-time only, no runtime cost)
        impl #struct_name {
            #[doc(hidden)]
            pub const __ORMADA_SCHEMA_TABLE: &'static str = #table_name;
            #[doc(hidden)]
            pub const __ORMADA_SCHEMA_MIGRATION: &'static str = #migration_id;
            #[doc(hidden)]
            pub const __ORMADA_SCHEMA_AFTER: &'static str = #after;
            #[doc(hidden)]
            pub const __ORMADA_SCHEMA_EXTENDS: &'static str = #extends;
        }
    };

    Ok(expanded)
}

/// Parse field attributes for schema definition
/// Returns attributes that affect schema (primary_key, foreign_key, index, etc.)
#[allow(dead_code)]
fn parse_schema_field_attrs(attrs: &[Attribute]) -> SchemaFieldConfig {
    let mut config = SchemaFieldConfig::default();

    for attr in attrs {
        let path = attr.path();
        let attr_name = path.get_ident().map(|i| i.to_string()).unwrap_or_default();

        match attr_name.as_str() {
            "primary_key" => config.primary_key = true,
            "foreign_key" => config.foreign_key = true,
            "index" => config.indexed = true,
            "unique" => config.unique = true,
            "nullable" => config.nullable = true,
            "max_length" => config.has_max_length = true,
            "default" => config.has_default = true,
            "soft_delete" => config.soft_delete = true,
            "rename" => config.is_rename = true,
            "drop" => config.is_drop = true,
            _ => {}
        }
    }

    config
}

#[derive(Default)]
struct SchemaFieldConfig {
    primary_key: bool,
    foreign_key: bool,
    indexed: bool,
    unique: bool,
    nullable: bool,
    has_max_length: bool,
    has_default: bool,
    soft_delete: bool,
    is_rename: bool,
    is_drop: bool,
}

/// Configuration for the `#[ormada_data_migration]` attribute
#[derive(Debug, Clone, Default)]
pub struct DataMigrationConfig {
    /// Migration ID
    pub migration_id: String,
    /// Migration this depends on
    pub after: Option<String>,
}

impl Parse for DataMigrationConfig {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut config = DataMigrationConfig::default();

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            match key.to_string().as_str() {
                "migration" => {
                    let value: syn::LitStr = input.parse()?;
                    config.migration_id = value.value();
                }
                "after" => {
                    let value: syn::LitStr = input.parse()?;
                    config.after = Some(value.value());
                }
                _ => {
                    return Err(syn::Error::new(key.span(), format!("Unknown attribute: {key}")));
                }
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        if config.migration_id.is_empty() {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "Missing required 'migration' attribute",
            ));
        }

        Ok(config)
    }
}

/// Implementation of the `#[ormada_data_migration]` attribute macro
pub fn impl_ormada_data_migration(
    attr: TokenStream,
    item: TokenStream,
) -> syn::Result<TokenStream> {
    let config: DataMigrationConfig = syn::parse2(attr)?;
    let input: syn::ItemFn = syn::parse2(item)?;

    let fn_name = &input.sig.ident;
    let fn_vis = &input.vis;
    let fn_block = &input.block;
    let fn_inputs = &input.sig.inputs;
    let fn_output = &input.sig.output;
    let fn_asyncness = &input.sig.asyncness;

    let migration_id = &config.migration_id;
    let after = config.after.as_deref().unwrap_or("");

    // Generate the function with metadata constants
    let expanded = quote! {
        #fn_vis #fn_asyncness fn #fn_name(#fn_inputs) #fn_output #fn_block

        // Metadata for CLI discovery
        #[doc(hidden)]
        pub mod __ormada_data_migration_meta {
            pub const MIGRATION_ID: &str = #migration_id;
            pub const AFTER: &str = #after;
            pub const FN_NAME: &str = stringify!(#fn_name);
        }
    };

    Ok(expanded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_schema_config() {
        let tokens: TokenStream = quote::quote! {
            table = "books", migration = "001_initial"
        };

        let config: SchemaConfig = syn::parse2(tokens).unwrap();
        assert_eq!(config.table_name, "books");
        assert_eq!(config.migration_id, Some("001_initial".to_string()));
    }

    #[test]
    fn test_parse_schema_config_with_extends() {
        let tokens: TokenStream = quote::quote! {
            table = "books", migration = "002", after = "001", extends = Book
        };

        let config: SchemaConfig = syn::parse2(tokens).unwrap();
        assert_eq!(config.table_name, "books");
        assert_eq!(config.migration_id, Some("002".to_string()));
        assert_eq!(config.after, Some("001".to_string()));
        assert_eq!(config.extends, Some("Book".to_string()));
    }
}
