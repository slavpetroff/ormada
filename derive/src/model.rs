///! Implementation of the `#[ergorm_model]` attribute macro
///!
///! This module provides the core functionality for transforming clean model definitions
///! into SeaORM-compatible code with ergonomic APIs.
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    Attribute, Data, DeriveInput, Expr, Fields, Ident, Lit, Meta, Token,
};

/// Configuration for the `#[ergorm_model]` attribute
#[derive(Debug, Clone)]
struct ModelConfig {
    table_name: String,
    composite_indexes: Vec<CompositeIndex>,
    ordering: Option<String>,
    /// If true, user will provide custom LifecycleHooks impl (don't auto-generate)
    /// Default: false (auto-generate empty impl)
    hooks: bool,
}

/// Composite index definition
#[derive(Debug, Clone)]
struct CompositeIndex {
    fields: Vec<String>,
    name: Option<String>,
}

/// Field-level attribute configuration
#[derive(Clone, Default)]
struct FieldConfig {
    // Primary key
    is_primary_key: bool,
    auto_increment: Option<bool>,

    // Foreign key
    foreign_key: Option<ForeignKeyConfig>,

    // Indexing
    index: Option<IndexConfig>,
    unique: Option<UniqueConfig>,

    // Validation
    max_length: Option<usize>,
    min_length: Option<usize>,
    range_min: Option<i64>,
    range_max: Option<i64>,

    // Timestamps
    auto_now: bool,
    auto_now_add: bool,

    // Soft delete
    soft_delete: bool,

    // Serialization
    skip_serializing: bool,
    skip_deserializing: bool,
}

#[derive(Clone)]
struct ForeignKeyConfig {
    entity: syn::Path, // Can be Author or super::author::Entity
    on_delete: Option<Ident>,
    default: Option<Expr>,
}

#[derive(Debug, Clone)]
struct IndexConfig {
    name: Option<String>,
}

#[derive(Debug, Clone)]
struct UniqueConfig {
    name: Option<String>,
}

impl Parse for ModelConfig {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut table_name = None;
        let composite_indexes = Vec::new();
        let mut ordering = None;
        let mut hooks = false; // Default: false (auto-generate empty impl)

        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            let _: Token![=] = input.parse()?;

            match ident.to_string().as_str() {
                "table" => {
                    let lit: Lit = input.parse()?;
                    if let Lit::Str(s) = lit {
                        table_name = Some(s.value());
                    }
                }
                "ordering" => {
                    let lit: Lit = input.parse()?;
                    if let Lit::Str(s) = lit {
                        ordering = Some(s.value());
                    }
                }
                "hooks" => {
                    let lit: Lit = input.parse()?;
                    if let Lit::Bool(b) = lit {
                        hooks = b.value();
                    }
                }
                _ => {
                    let ident_str = ident.to_string();
                    return Err(syn::Error::new_spanned(
                        ident,
                        format!("Unknown attribute: {}", ident_str),
                    ));
                }
            }

            if input.peek(Token![,]) {
                let _: Token![,] = input.parse()?;
            }
        }

        Ok(ModelConfig {
            table_name: table_name.ok_or_else(|| {
                syn::Error::new(input.span(), "Missing required 'table' attribute")
            })?,
            composite_indexes,
            ordering,
            hooks,
        })
    }
}

/// Parse field attributes to extract configuration
fn parse_field_attributes(attrs: &[Attribute]) -> syn::Result<FieldConfig> {
    let mut config = FieldConfig::default();

    for attr in attrs {
        if !attr.path().is_ident("primary_key")
            && !attr.path().is_ident("foreign_key")
            && !attr.path().is_ident("index")
            && !attr.path().is_ident("unique")
            && !attr.path().is_ident("max_length")
            && !attr.path().is_ident("min_length")
            && !attr.path().is_ident("range")
            && !attr.path().is_ident("auto_now")
            && !attr.path().is_ident("auto_now_add")
            && !attr.path().is_ident("soft_delete")
            && !attr.path().is_ident("skip_serializing")
            && !attr.path().is_ident("skip_deserializing")
        {
            continue;
        }

        match attr.meta {
            Meta::Path(ref path) if path.is_ident("primary_key") => {
                config.is_primary_key = true;
            }
            Meta::Path(ref path) if path.is_ident("index") => {
                config.index = Some(IndexConfig { name: None });
            }
            Meta::Path(ref path) if path.is_ident("unique") => {
                config.unique = Some(UniqueConfig { name: None });
            }
            Meta::Path(ref path) if path.is_ident("auto_now") => {
                config.auto_now = true;
            }
            Meta::Path(ref path) if path.is_ident("auto_now_add") => {
                config.auto_now_add = true;
            }
            Meta::Path(ref path) if path.is_ident("soft_delete") => {
                config.soft_delete = true;
            }
            Meta::Path(ref path) if path.is_ident("skip_serializing") => {
                config.skip_serializing = true;
            }
            Meta::Path(ref path) if path.is_ident("skip_deserializing") => {
                config.skip_deserializing = true;
            }
            Meta::List(ref meta_list) => {
                let path = &meta_list.path;

                if path.is_ident("primary_key") {
                    config.is_primary_key = true;
                    // Parse options like auto_increment
                    meta_list.parse_nested_meta(|meta| {
                        if meta.path.is_ident("auto_increment") {
                            let _: Token![=] = meta.input.parse()?;
                            let lit: Lit = meta.input.parse()?;
                            if let Lit::Bool(b) = lit {
                                config.auto_increment = Some(b.value);
                            }
                        }
                        Ok(())
                    })?;
                } else if path.is_ident("foreign_key") {
                    // Parse foreign_key(Entity, on_delete = Cascade)
                    config.foreign_key = Some(parse_foreign_key(meta_list)?);
                } else if path.is_ident("max_length") {
                    // Parse #[max_length(50)] - direct literal argument
                    let lit: Lit = meta_list.parse_args()?;
                    if let Lit::Int(i) = lit {
                        config.max_length = Some(i.base10_parse()?);
                    }
                } else if path.is_ident("min_length") {
                    // Parse #[min_length(5)] - direct literal argument
                    let lit: Lit = meta_list.parse_args()?;
                    if let Lit::Int(i) = lit {
                        config.min_length = Some(i.base10_parse()?);
                    }
                } else if path.is_ident("range") {
                    meta_list.parse_nested_meta(|meta| {
                        if meta.path.is_ident("min") {
                            let _: Token![=] = meta.input.parse()?;
                            let lit: Lit = meta.input.parse()?;
                            if let Lit::Int(i) = lit {
                                config.range_min = Some(i.base10_parse()?);
                            }
                        } else if meta.path.is_ident("max") {
                            let _: Token![=] = meta.input.parse()?;
                            let lit: Lit = meta.input.parse()?;
                            if let Lit::Int(i) = lit {
                                config.range_max = Some(i.base10_parse()?);
                            }
                        }
                        Ok(())
                    })?;
                } else if path.is_ident("index") {
                    let mut name = None;
                    meta_list.parse_nested_meta(|meta| {
                        if meta.path.is_ident("name") {
                            let _: Token![=] = meta.input.parse()?;
                            let lit: Lit = meta.input.parse()?;
                            if let Lit::Str(s) = lit {
                                name = Some(s.value());
                            }
                        }
                        Ok(())
                    })?;
                    config.index = Some(IndexConfig { name });
                } else if path.is_ident("unique") {
                    let mut name = None;
                    meta_list.parse_nested_meta(|meta| {
                        if meta.path.is_ident("name") {
                            let _: Token![=] = meta.input.parse()?;
                            let lit: Lit = meta.input.parse()?;
                            if let Lit::Str(s) = lit {
                                name = Some(s.value());
                            }
                        }
                        Ok(())
                    })?;
                    config.unique = Some(UniqueConfig { name });
                }
            }
            _ => {}
        }
    }

    Ok(config)
}

fn parse_foreign_key(meta_list: &syn::MetaList) -> syn::Result<ForeignKeyConfig> {
    let mut model_type = None;
    let mut on_delete = None;
    let mut default = None;

    meta_list.parse_nested_meta(|meta| {
        if model_type.is_none() {
            // First positional argument is the Model type (e.g., Author)
            // User should NEVER provide Entity - we auto-convert Model -> Entity
            let path = meta.path.clone();

            // Convert Model type to Entity path
            // From _internal module context, we need super::super to reach sibling modules
            // Author -> super::super::author::_internal::Entity
            let entity_path = if path.segments.len() == 1 {
                // Simple case: Author -> super::super::author::_internal::Entity
                let model_name = &path.segments[0].ident;
                let module_name = format_ident!("{}", to_snake_case(&model_name.to_string()));
                syn::parse_quote! { super::super::#module_name::_internal::Entity }
            } else {
                // Path case: needs more complex handling
                // For now, assume simple case is most common
                let model_name =
                    &path.segments.last().map(|seg| &seg.ident).expect("Path must have segments");
                let module_name = format_ident!("{}", to_snake_case(&model_name.to_string()));
                syn::parse_quote! { super::super::#module_name::_internal::Entity }
            };

            model_type = Some(entity_path);
        } else if meta.path.is_ident("on_delete") {
            let _: Token![=] = meta.input.parse()?;
            on_delete = Some(meta.input.parse::<Ident>()?);
        } else if meta.path.is_ident("default") {
            let _: Token![=] = meta.input.parse()?;
            default = Some(meta.input.parse::<Expr>()?);
        }
        Ok(())
    })?;

    Ok(ForeignKeyConfig {
        entity: model_type.ok_or_else(|| {
            syn::Error::new_spanned(meta_list, "foreign_key requires a Model type")
        })?,
        on_delete,
        default,
    })
}

/// Main implementation of the ergorm_model attribute macro
pub fn impl_ergorm_model(attr: TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
    let config: ModelConfig = syn::parse2(attr)?;
    let mut input: DeriveInput = syn::parse2(input)?;

    let struct_name = &input.ident;
    let table_name = &config.table_name;

    // Extract fields
    let fields = match &mut input.data {
        Data::Struct(data) => match &mut data.fields {
            Fields::Named(fields) => fields,
            _ => {
                return Err(syn::Error::new_spanned(
                    struct_name,
                    "django_model only supports structs with named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(struct_name, "django_model only supports structs"));
        }
    };

    // Parse field configurations and strip our custom attributes
    let mut field_configs = Vec::new();
    let mut has_primary_key = false;
    let mut primary_key_fields = Vec::new();
    let mut foreign_keys = Vec::new();
    let mut soft_delete_field: Option<Ident> = None;

    for field in fields.named.iter_mut() {
        let config = parse_field_attributes(&field.attrs)?;

        // Get field identifier early - all named fields must have idents
        let field_ident = match &field.ident {
            Some(ident) => ident.clone(),
            None => return Err(syn::Error::new_spanned(&*field, "Field must have a name")),
        };

        let field_type = field.ty.clone();

        // Validation
        if config.is_primary_key {
            has_primary_key = true;
            primary_key_fields.push(field_ident.clone());
        }
        if let Some(ref fk) = config.foreign_key {
            foreign_keys.push((field_ident.clone(), fk.clone()));
        }
        if config.soft_delete {
            if soft_delete_field.is_some() {
                return Err(syn::Error::new(
                    field_ident.span(),
                    "Only one field can be marked with #[soft_delete]",
                ));
            }
            soft_delete_field = Some(field_ident.clone());
        }

        // Store config before stripping attributes
        field_configs.push((field_ident, field_type, config.clone()));

        // Strip our custom attributes, keep only SeaORM/serde ones
        strip_django_attributes(field, &config);
    }

    if !has_primary_key {
        return Err(syn::Error::new_spanned(
            struct_name,
            "Model must have at least one field marked with #[primary_key]",
        ));
    }

    // Validate SetNull foreign keys require Option<T>
    for (field_name, field_type, config) in &field_configs {
        if let Some(ref fk_config) = config.foreign_key {
            if let Some(ref on_delete) = fk_config.on_delete {
                if on_delete == "SetNull" {
                    let type_str = quote!(#field_type).to_string();
                    if !type_str.contains("Option") {
                        return Err(syn::Error::new(
                            field_name.span(),
                            "on_delete = SetNull requires field type to be Option<T>",
                        ));
                    }
                }
            }
        }
    }

    // Add necessary derives to the struct
    // Note: DeriveEntityModel generates Entity, Column, PrimaryKey, ActiveModel
    // Note: We don't derive Default here because we inject relation fields later
    // Use the sea_orm derive through our internal module
    input.attrs.push(syn::parse_quote! {
        #[derive(Clone, Debug, PartialEq, Eq, ::seaorm_django::__internal::sea_orm::DeriveEntityModel)]
    });

    // Add sea_orm table_name attribute
    input.attrs.push(syn::parse_quote! {
        #[sea_orm(table_name = #table_name)]
    });

    // Keep original name for convenience alias
    let original_name = input.ident.clone();

    // Generate snake_case module name from struct name
    let module_name = format_ident!("{}", to_snake_case(&original_name.to_string()));

    // Rename struct to Model (SeaORM convention) and make it public
    input.ident = format_ident!("Model");
    input.vis = syn::Visibility::Public(syn::token::Pub::default());

    // Inject relation fields
    if let syn::Data::Struct(ref mut data) = input.data {
        if let syn::Fields::Named(ref mut fields) = data.fields {
            for (field_ident, fk) in &foreign_keys {
                let field_name_str = field_ident.to_string();
                let relation_name_str = if field_name_str.ends_with("_id") {
                    &field_name_str[..field_name_str.len() - 3]
                } else {
                    &field_name_str
                };
                let relation_name = format_ident!("{}", relation_name_str);
                let relation_type = &fk.entity; // This is a Path to Entity

                // Add field: pub relation_name: Option<Model>
                // We use the Entity::Model type
                fields.named.push(syn::Field {
                    attrs: vec![syn::parse_quote! { #[sea_orm(ignore)] }],
                    vis: syn::Visibility::Public(syn::token::Pub::default()),
                    mutability: syn::FieldMutability::None,
                    ident: Some(relation_name),
                    colon_token: Some(syn::token::Colon::default()),
                    ty: syn::parse_quote! { ::core::option::Option<<#relation_type as ::seaorm_django::__internal::EntityTrait>::Model> },
                });
            }
        }
    }

    // Generate additional components
    let relation_enum = generate_relation_enum(&foreign_keys);
    let entity_impl = generate_entity_impl();
    let django_entity_impl =
        generate_django_entity_impl(&field_configs, table_name, soft_delete_field.as_ref())?;
    let has_relation_impls = generate_has_relation_impls(&foreign_keys);
    let with_relations_trait_impl = generate_with_relations_trait(&foreign_keys);
    let model_save_impl = generate_model_save_impl(&field_configs)?;
    let model_delete_impl = generate_model_delete_impl(soft_delete_field.as_ref())?;
    let model_convenience_impl = generate_model_convenience_methods(&input, &config.ordering)?;
    let default_impl = generate_default_impl(&field_configs, &foreign_keys);

    // Generate default LifecycleHooks impl only if hooks = false (default)
    // hooks = true means user will provide custom implementation
    let lifecycle_hooks_impl = if config.hooks {
        // User will provide their own implementation
        quote! {}
    } else {
        // Auto-generate default no-op implementation
        quote! {
            // Default LifecycleHooks implementation (all hooks are no-ops)
            // Use `#[django_model(table = "...", hooks = true)]` to provide custom hooks
            #[::async_trait::async_trait]
            impl ::seaorm_django::hooks::LifecycleHooks for Model {}
        }
    };

    // Generate code with nested module to avoid conflicts
    // This creates the internal SeaORM types and exposes Model as the main interface
    let expanded = quote! {
        // Internal module for SeaORM compatibility - users don't touch this
        // pub(crate) allows other models to reference Entity for relations
        pub(crate) mod _internal {
            use ::serde::{Serialize, Deserialize};
            // Use sea_orm re-exported through seaorm_django to avoid requiring direct dependency
            // All types come from seaorm_django's internal module
            use ::seaorm_django::__internal::sea_orm::entity::prelude::*;
            use ::seaorm_django::__internal::*;
            use ::seaorm_django::prelude::DateTimeWithTimeZone;
            use ::seaorm_django::types::OnDelete;

            // The Model struct with DeriveEntityModel (this generates Entity internally)
            #input

            // Relation enum
            #relation_enum

            // ActiveModelBehavior implementation
            #entity_impl

            // Django entity trait implementation
            #django_entity_impl

            // HasRelation implementations for foreign keys
            #has_relation_impls

            // WithRelationsTrait implementation (required for relations system)
            #with_relations_trait_impl

            // Default implementation that handles injected relation fields
            #default_impl
        }

        // Export Model as the primary type - this is what users work with!
        pub use _internal::Model;

        // Internal types are pub(crate) so other models can reference Entity for relations
        // but end users never see them directly
        pub(crate) use _internal::{Entity, ActiveModel, Column, PrimaryKey, Relation};

        // Alias for convenience in generated code
        use _internal::Entity as _Entity;

        // Model instance methods (save, delete, etc.)
        #model_save_impl
        #model_delete_impl

        // Model static methods and column constants
        #model_convenience_impl

        // Implement HasEntityType trait so relations! macro can extract Entity from Model
        impl ::seaorm_django::relations::HasEntityType for Model {
            type __Entity = Entity;
        }

        // LifecycleHooks implementation (auto-generated unless hooks = "custom")
        #lifecycle_hooks_impl

        // Forward ErgormEntity methods to Entity
        impl Model {
            /// Validate and convert Model to ActiveModel for creation
            ///
            /// This is a convenience method that forwards to the Entity implementation.
            pub fn to_active_model_for_create(model: Self) -> ::core::result::Result<ActiveModel, ::seaorm_django::error::ErgormError> {
                <Entity as ::seaorm_django::traits::ErgormEntity>::to_active_model_for_create(model)
            }
        }

        // Main export: Author = Model (the data struct users work with)
        pub type #original_name = Model;
    };

    Ok(expanded)
}

/// Convert PascalCase to snake_case
fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c.is_uppercase() {
            if !result.is_empty() {
                result.push('_');
            }
            result.push(c.to_lowercase().next().unwrap());
        } else {
            result.push(c);
        }
    }

    result
}

/// Strip django-specific attributes from a field, keeping only SeaORM/serde ones
fn strip_django_attributes(field: &mut syn::Field, config: &FieldConfig) {
    // Make field public
    field.vis = syn::Visibility::Public(syn::token::Pub::default());

    // Keep only attributes that SeaORM and serde understand
    let mut new_attrs = Vec::new();

    for attr in &field.attrs {
        // Keep doc comments and other non-django attributes
        if !attr.path().is_ident("primary_key")
            && !attr.path().is_ident("foreign_key")
            && !attr.path().is_ident("index")
            && !attr.path().is_ident("unique")
            && !attr.path().is_ident("max_length")
            && !attr.path().is_ident("min_length")
            && !attr.path().is_ident("range")
            && !attr.path().is_ident("auto_now")
            && !attr.path().is_ident("auto_now_add")
            && !attr.path().is_ident("soft_delete")
            && !attr.path().is_ident("skip_serializing")
            && !attr.path().is_ident("skip_deserializing")
        {
            new_attrs.push(attr.clone());
        }
    }

    // Add SeaORM attributes based on config
    if config.is_primary_key {
        new_attrs.push(syn::parse_quote! { #[sea_orm(primary_key)] });
    }
    if config.index.is_some() {
        new_attrs.push(syn::parse_quote! { #[sea_orm(indexed)] });
    }
    if config.unique.is_some() {
        new_attrs.push(syn::parse_quote! { #[sea_orm(unique)] });
    }

    // TODO: Add serde attributes back when we properly handle serde dependency
    // if config.skip_deserializing {
    //     new_attrs.push(syn::parse_quote! { #[serde(skip_deserializing)] });
    // }
    // if config.skip_serializing {
    //     new_attrs.push(syn::parse_quote! { #[serde(skip_serializing)] });
    // }

    field.attrs = new_attrs;
}

fn generate_model_struct(
    field_configs: &[(&syn::Field, FieldConfig)],
    table_name: &str,
) -> syn::Result<TokenStream> {
    let mut fields = Vec::new();

    for (field, config) in field_configs {
        let field_name = &field.ident;
        let field_type = &field.ty;

        // Only include attributes that SeaORM and serde understand
        // Our custom attributes (max_length, etc.) are stripped
        let mut field_attrs = Vec::new();

        // Primary key attribute for SeaORM
        if config.is_primary_key {
            field_attrs.push(quote! { #[sea_orm(primary_key)] });
        }

        // Serialization attributes for serde
        if config.skip_deserializing {
            field_attrs.push(quote! { #[serde(skip_deserializing)] });
        }
        if config.skip_serializing {
            field_attrs.push(quote! { #[serde(skip_serializing)] });
        }

        fields.push(quote! {
            #(#field_attrs)*
            pub #field_name: #field_type,
        });
    }

    Ok(quote! {
        #[derive(Clone, Debug, PartialEq, Eq, ::seaorm_django::__internal::sea_orm::DeriveEntityModel, ::serde::Serialize, ::serde::Deserialize, Default)]
        #[sea_orm(table_name = #table_name)]
        pub struct Model {
            #(#fields)*
        }
    })
}

fn generate_column_enum(field_configs: &[(&syn::Field, FieldConfig)]) -> TokenStream {
    let variants: Vec<_> = field_configs
        .iter()
        .map(|(field, _)| {
            let name = field.ident.as_ref().unwrap();
            let variant_name = format_ident!("{}", to_pascal_case(&name.to_string()));
            variant_name
        })
        .collect();

    quote! {
        #[derive(Copy, Clone, Debug, ::seaorm_django::__internal::sea_orm::EnumIter, ::seaorm_django::__internal::sea_orm::DeriveColumn)]
        pub enum Column {
            #(#variants,)*
        }
    }
}

fn generate_primary_key_enum(primary_key_fields: &[Ident]) -> TokenStream {
    let variants: Vec<_> = primary_key_fields
        .iter()
        .map(|name| {
            let variant_name = format_ident!("{}", to_pascal_case(&name.to_string()));
            variant_name
        })
        .collect();

    quote! {
        #[derive(Copy, Clone, Debug, ::seaorm_django::__internal::sea_orm::EnumIter, ::seaorm_django::__internal::sea_orm::DerivePrimaryKey)]
        pub enum PrimaryKey {
            #(#variants,)*
        }

        impl ::seaorm_django::__internal::PrimaryKeyTrait for PrimaryKey {
            type ValueType = i32; // TODO: Detect actual type
        }
    }
}

fn generate_relation_enum(foreign_keys: &[(Ident, ForeignKeyConfig)]) -> TokenStream {
    if foreign_keys.is_empty() {
        return quote! {
            #[derive(Copy, Clone, Debug, ::seaorm_django::__internal::sea_orm::EnumIter, ::seaorm_django::__internal::sea_orm::DeriveRelation)]
            pub enum Relation {}
        };
    }

    let variants: Vec<_> = foreign_keys
        .iter()
        .map(|(field_name, fk)| {
            // Extract meaningful name from the path
            // e.g., crate::author::author::Entity -> use second-to-last "author"
            // or super::author::Entity -> use "author"
            let segments: Vec<_> = fk.entity.segments.iter().collect();
            let variant_name = if segments.len() >= 2 && segments.last().unwrap().ident == "Entity"
            {
                // Path ends with ::Entity, use the segment before it
                &segments[segments.len() - 2].ident
            } else {
                // Otherwise use last segment
                &segments.last().unwrap().ident
            };

            // Generate Column name (PascalCase from field_name)
            let column_name_str = to_pascal_case(&field_name.to_string());

            // Convert entity path to string for all three attributes
            let entity_path = &fk.entity;
            let entity_path_str = quote!(#entity_path).to_string().replace(" ", "");

            // For "to" path, we need to reference Column in the same module as Entity
            // e.g., "crate::author::Entity" -> "crate::author::Column::Id"
            // Remove "::Entity" suffix and add "::Column::Id"
            let to_path_str = if entity_path_str.ends_with("::Entity") {
                // Replace ::Entity with ::Column::Id
                format!("{}::Column::Id", &entity_path_str[..entity_path_str.len() - 8])
            } else {
                // Fallback: just append ::Column::Id
                format!("{}::Column::Id", entity_path_str)
            };

            // All three values must be string literals
            let belongs_to_lit = syn::LitStr::new(&entity_path_str, proc_macro2::Span::call_site());
            let from_path = syn::LitStr::new(
                &format!("Column::{}", column_name_str),
                proc_macro2::Span::call_site(),
            );
            let to_path = syn::LitStr::new(&to_path_str, proc_macro2::Span::call_site());

            quote! {
                #[sea_orm(
                    belongs_to = #belongs_to_lit,
                    from = #from_path,
                    to = #to_path
                )]
                #variant_name
            }
        })
        .collect();

    quote! {
        #[derive(Copy, Clone, Debug, ::seaorm_django::__internal::sea_orm::EnumIter, ::seaorm_django::__internal::sea_orm::DeriveRelation)]
        pub enum Relation {
            #(#variants,)*
        }
    }
}

fn generate_entity_impl() -> TokenStream {
    quote! {
        impl ::seaorm_django::__internal::ActiveModelBehavior for ActiveModel {}
    }
}

fn generate_django_entity_impl(
    field_configs: &[(Ident, syn::Type, FieldConfig)],
    table_name: &str,
    soft_delete_field: Option<&Ident>,
) -> syn::Result<TokenStream> {
    let mut create_assignments = Vec::new();
    let mut validations = Vec::new();

    for (field_name, field_type, config) in field_configs {
        // Generate validation code
        let field_name_str = field_name.to_string();

        // String length validations
        if config.max_length.is_some() || config.min_length.is_some() {
            let type_str = quote!(#field_type).to_string();
            if type_str.contains("String") {
                if let Some(max) = config.max_length {
                    validations.push(quote! {
                        if model.#field_name.len() > #max {
                            return ::core::result::Result::Err(
                                ::seaorm_django::error::ErgormError::validation(
                                    #table_name,
                                    #field_name_str,
                                    ::std::format!("exceeds max_length of {}", #max)
                                )
                            );
                        }
                    });
                }
                if let Some(min) = config.min_length {
                    validations.push(quote! {
                        if model.#field_name.len() < #min {
                            return ::core::result::Result::Err(
                                ::seaorm_django::error::ErgormError::validation(
                                    #table_name,
                                    #field_name_str,
                                    ::std::format!("is shorter than min_length of {}", #min)
                                )
                            );
                        }
                    });
                }
            }
        }

        // Numeric range validations
        if config.range_min.is_some() || config.range_max.is_some() {
            if let Some(min) = config.range_min {
                // Cast to field type to avoid type mismatches
                validations.push(quote! {
                    if (model.#field_name as i64) < #min {
                        return ::core::result::Result::Err(
                            ::seaorm_django::error::ErgormError::validation(
                                #table_name,
                                #field_name_str,
                                ::std::format!("value {} is less than minimum {}", model.#field_name, #min)
                            )
                        );
                    }
                });
            }
            if let Some(max) = config.range_max {
                // Cast to field type to avoid type mismatches
                validations.push(quote! {
                    if (model.#field_name as i64) > #max {
                        return ::core::result::Result::Err(
                            ::seaorm_django::error::ErgormError::validation(
                                #table_name,
                                #field_name_str,
                                ::std::format!("value {} exceeds maximum {}", model.#field_name, #max)
                            )
                        );
                    }
                });
            }
        }

        // Generate field assignment
        if config.auto_now_add || config.auto_now {
            create_assignments.push(quote! {
                #field_name: ::seaorm_django::__internal::Set(now)
            });
        } else if config.is_primary_key {
            create_assignments.push(quote! {
                #field_name: ::seaorm_django::__internal::NotSet
            });
        } else if let Some(ref fk) = config.foreign_key {
            create_assignments.push(quote! {
                #field_name: ::seaorm_django::__internal::Set(model.#field_name)
            });
        } else {
            // Regular field
            create_assignments.push(quote! {
                #field_name: ::seaorm_django::__internal::Set(model.#field_name)
            });
        }
    }

    // Generate soft_delete implementation using enum
    let soft_delete_impl = if let Some(field) = soft_delete_field {
        let field_str = field.to_string();
        quote! {
            fn soft_delete() -> ::seaorm_django::traits::SoftDeleteConfig {
                ::seaorm_django::traits::SoftDeleteConfig::Enabled { column: #field_str }
            }
        }
    } else {
        quote! {}
    };

    Ok(quote! {
        impl ::seaorm_django::traits::ErgormEntity for Entity {
            fn to_active_model_for_create(model: Model) -> ::core::result::Result<ActiveModel, ::seaorm_django::error::ErgormError> {
                // Validation logic
                #(#validations)*

                let now = ::seaorm_django::__internal::Utc::now().fixed_offset();
                ::core::result::Result::Ok(ActiveModel {
                    #(#create_assignments,)*
                })
            }

            async fn save_model<'a, C: ::seaorm_django::__internal::ConnectionTrait>(
                db: &'a C,
                model: Model,
            ) -> ::core::result::Result<Model, ::seaorm_django::error::ErgormError> {
                model.save(db).await
            }

            #soft_delete_impl
        }
    })
}

/// Generate WithRelationsTrait implementation
///
/// This trait is ALWAYS generated (even for entities without foreign keys)
/// so that the relations system works properly.
fn generate_with_relations_trait(foreign_keys: &[(Ident, ForeignKeyConfig)]) -> TokenStream {
    // For now, we generate a minimal implementation with no relations
    // In the future, this could be enhanced to support actual relation loading
    quote! {
        impl ::seaorm_django::traits::WithRelationsTrait for Entity {
            type Model = Model;
            type ModelWithRelations = Model; // For now, same as Model

            fn from_model_and_relations<R>(
                model: Self::Model,
                _relations: &R,
            ) -> Self::ModelWithRelations {
                model // Just return the model as-is
            }
        }
    }
}

fn generate_has_relation_impls(foreign_keys: &[(Ident, ForeignKeyConfig)]) -> TokenStream {
    let impls: Vec<_> = foreign_keys
        .iter()
        .map(|(field_name, fk)| {
            let entity = &fk.entity;
            let relation_name_str = if field_name.to_string().ends_with("_id") {
                &field_name.to_string()[..field_name.to_string().len() - 3]
            } else {
                &field_name.to_string()
            };
            let relation_name = format_ident!("{}", relation_name_str);

            quote! {
                impl ::seaorm_django::relations::HasRelation<#entity> for Entity {
                    type RelatedPK = i32; // TODO: Detect actual type

                    fn get_foreign_key(model: &Self::Model) -> Self::RelatedPK {
                        model.#field_name
                    }

                    fn set_related(model: &mut Self::Model, related: ::core::option::Option<<#entity as ::seaorm_django::__internal::EntityTrait>::Model>) {
                        model.#relation_name = related;
                    }

                    async fn load_related<C: ::seaorm_django::__internal::ConnectionTrait>(
                        models: &[Self::Model],
                        db: &C,
                    ) -> ::core::result::Result<
                        ::seaorm_django::prelude::FxHashMap<Self::RelatedPK, <#entity as ::seaorm_django::__internal::EntityTrait>::Model>,
                        ::seaorm_django::error::ErgormError
                    > {
                        use ::seaorm_django::__internal::{EntityTrait, QueryFilter, ColumnTrait, Iterable};

                        let fk_values: ::std::vec::Vec<Self::RelatedPK> = models
                            .iter()
                            .map(|m| m.#field_name)
                            .collect();

                        if fk_values.is_empty() {
                            return ::core::result::Result::Ok(::seaorm_django::prelude::FxHashMap::default());
                        }

                        let pk_cols: ::std::vec::Vec<_> = <#entity as ::seaorm_django::__internal::EntityTrait>::PrimaryKey::iter()
                            .map(|pk| pk.into_column())
                            .collect();
                        let id_column = pk_cols[0];

                        let related_models = <#entity as ::seaorm_django::__internal::EntityTrait>::find()
                            .filter(id_column.is_in(fk_values))
                            .all(db)
                            .await?;

                        let mut map = ::seaorm_django::prelude::FxHashMap::default();
                        for model in related_models {
                            let key = model.id;
                            map.insert(key, model);
                        }

                        ::core::result::Result::Ok(map)
                    }
                }
            }
        })
        .collect();

    quote! { #(#impls)* }
}

/// Generate the save method for the model
fn generate_model_save_impl(
    field_configs: &[(Ident, syn::Type, FieldConfig)],
) -> syn::Result<TokenStream> {
    let auto_now_updates = field_configs.iter().filter_map(|(ident, _, config)| {
        if config.auto_now {
            Some(quote! {
                active_model.#ident = ::seaorm_django::__internal::Set(::seaorm_django::__internal::Utc::now().fixed_offset());
            })
        } else {
            None
        }
    });

    // Force all fields to Set to ensure they are updated
    // ActiveModel::from() sets them to Unchanged, which causes save() to skip updating them
    let force_set_updates = field_configs.iter().filter_map(|(ident, _, config)| {
        if !config.is_primary_key && !config.auto_now {
            Some(quote! {
                active_model.#ident = ::seaorm_django::__internal::Set(active_model.#ident.unwrap());
            })
        } else {
            None
        }
    });

    Ok(quote! {
        impl Model {
            /// Save the model to the database.
            ///
            /// If the model has a primary key set and exists, it updates.
            /// Otherwise, it inserts.
            /// Handles `auto_now` fields automatically.
            /// Triggers `before_save` and `after_save` hooks.
            pub async fn save<'a, C: ::seaorm_django::__internal::ConnectionTrait>(
                mut self,
                db: &'a C,
            ) -> ::core::result::Result<Self, ::seaorm_django::error::ErgormError> {
                use ::seaorm_django::hooks::LifecycleHooks;
                use ::seaorm_django::__internal::ActiveModelTrait;
                use ::seaorm_django::__internal::TryIntoModel;

                // Pre-save hooks
                <Self as LifecycleHooks>::before_save(&mut self).await?;

                // Convert to ActiveModel
                let mut active_model = ActiveModel::from(self);

                // Ensure all fields are marked as Set so they get updated
                #(#force_set_updates)*

                // Update auto_now fields
                #(#auto_now_updates)*

                // Save (Insert or Update)
                let result = active_model.save(db).await?;

                // Convert back to Model
                let model = result.try_into_model()?;

                // Post-save hooks
                <Self as LifecycleHooks>::after_save(&model).await?;

                ::core::result::Result::Ok(model)
            }
        }
    })
}

/// Generate delete methods with soft delete support
fn generate_model_delete_impl(soft_delete_field: Option<&Ident>) -> syn::Result<TokenStream> {
    if let Some(field_name) = soft_delete_field {
        // Soft delete implementation
        Ok(quote! {
            impl Model {
                /// Soft delete this model (sets deleted_at timestamp).
                ///
                /// The record remains in the database but is excluded from queries by default.
                /// Use `.with_deleted()` to include soft-deleted records in queries.
                /// Use `.restore()` to un-delete a soft-deleted record.
                pub async fn delete<C: ::seaorm_django::__internal::ConnectionTrait>(
                    mut self,
                    db: &C,
                ) -> ::core::result::Result<Self, ::seaorm_django::error::ErgormError> {
                    use ::seaorm_django::__internal::{ActiveModelTrait, Set, ActiveValue};
                    use ::seaorm_django::hooks::LifecycleHooks;

                    <Self as LifecycleHooks>::before_delete(&self).await?;

                    // Convert to ActiveModel and set deleted_at
                    let mut active = ActiveModel::from(self);
                    active.#field_name = Set(::core::option::Option::Some(::seaorm_django::__internal::Utc::now().fixed_offset()));

                    // Update in database
                    let updated = active.update(db).await?;

                    <Self as LifecycleHooks>::after_delete(&updated).await?;

                    ::core::result::Result::Ok(updated)
                }

                /// Permanently delete this record from the database (hard delete).
                ///
                /// This cannot be undone. Use `.delete()` for soft delete instead.
                pub async fn force_delete<C: ::seaorm_django::__internal::ConnectionTrait>(
                    mut self,
                    db: &C,
                ) -> ::core::result::Result<(), ::seaorm_django::error::ErgormError> {
                    use ::seaorm_django::__internal::ActiveModelTrait;
                    use ::seaorm_django::hooks::LifecycleHooks;

                    <Self as LifecycleHooks>::before_delete(&self).await?;

                    let active = ActiveModel::from(self.clone());
                    active.delete(db).await?;

                    <Self as LifecycleHooks>::after_delete(&self).await?;

                    ::core::result::Result::Ok(())
                }

                /// Restore a soft-deleted record (set deleted_at to NULL).
                ///
                /// Makes the record visible in queries again.
                pub async fn restore<C: ::seaorm_django::__internal::ConnectionTrait>(
                    mut self,
                    db: &C,
                ) -> ::core::result::Result<Self, ::seaorm_django::error::ErgormError> {
                    use ::seaorm_django::__internal::{ActiveModelTrait, Set, ActiveValue};
                    use ::seaorm_django::hooks::LifecycleHooks;

                    // TODO: Should we have before_restore hooks?
                    // For now, treat it as an update
                    <Self as LifecycleHooks>::before_save(&mut self).await?;
                    <Self as LifecycleHooks>::before_update(&mut self).await?;

                    // Convert to ActiveModel and set deleted_at to NULL
                    let mut active = ActiveModel::from(self);
                    active.#field_name = Set(::core::option::Option::None);

                    // Update in database
                    let updated = active.update(db).await?;

                    <Self as LifecycleHooks>::after_update(&updated).await?;
                    <Self as LifecycleHooks>::after_save(&updated).await?;

                    ::core::result::Result::Ok(updated)
                }
            }
        })
    } else {
        // Hard delete implementation (no soft deletes)
        Ok(quote! {
            impl Model {
                /// Delete this record from the database.
                pub async fn delete<C: ::seaorm_django::__internal::ConnectionTrait>(
                    mut self,
                    db: &C,
                ) -> ::core::result::Result<(), ::seaorm_django::error::ErgormError> {
                    use ::seaorm_django::__internal::ActiveModelTrait;
                    use ::seaorm_django::hooks::LifecycleHooks;

                    <Self as LifecycleHooks>::before_delete(&self).await?;

                    let active = ActiveModel::from(self.clone());
                    active.delete(db).await?;

                    <Self as LifecycleHooks>::after_delete(&self).await?;

                    ::core::result::Result::Ok(())
                }
            }
        })
    }
}

/// Generate convenience methods and constants on Model for better UX
fn generate_model_convenience_methods(
    input: &DeriveInput,
    ordering: &Option<String>,
) -> syn::Result<TokenStream> {
    // Extract field names to generate column constants
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    input,
                    "Only structs with named fields are supported",
                ))
            }
        },
        _ => return Err(syn::Error::new_spanned(input, "Only structs are supported")),
    };

    // Generate column constants: Book::Title = Column::Title
    let column_constants: Vec<_> = fields
        .iter()
        .filter(|field| {
            // Filter out fields with #[sea_orm(ignore)]
            !field.attrs.iter().any(|attr| {
                if attr.path().is_ident("sea_orm") {
                    let attr_str = quote!(#attr).to_string();
                    attr_str.contains("ignore")
                } else {
                    false
                }
            })
        })
        .map(|field| {
            let field_name = field.ident.as_ref().unwrap();
            let constant_name = format_ident!("{}", to_pascal_case(&field_name.to_string()));
            quote! {
                pub const #constant_name: Column = Column::#constant_name;
            }
        })
        .collect();

    // Generate default ordering method if ordering is specified
    let default_ordering_method = if let Some(ordering_str) = ordering {
        let desc = ordering_str.starts_with('-');
        let column_name = ordering_str.trim_start_matches('-');
        let column_ident = format_ident!("{}", to_pascal_case(column_name));

        if desc {
            quote! {
                /// Apply default ordering (from #[django_model(ordering = "...")])
                pub fn default_ordering<C: ::seaorm_django::__internal::ConnectionTrait>(
                    db: &C,
                ) -> ::seaorm_django::query::QuerySet<'_, _Entity, C> {
                    use ::seaorm_django::query::QueryExt;
                    _Entity::objects(db).order_by_desc(Self::#column_ident)
                }
            }
        } else {
            quote! {
                /// Apply default ordering (from #[django_model(ordering = "...")])
                pub fn default_ordering<C: ::seaorm_django::__internal::ConnectionTrait>(
                    db: &C,
                ) -> ::seaorm_django::query::QuerySet<'_, _Entity, C> {
                    use ::seaorm_django::query::QueryExt;
                    _Entity::objects(db).order_by_asc(Self::#column_ident)
                }
            }
        }
    } else {
        quote! {}
    };

    Ok(quote! {
        impl Model {
            // Column constants for convenient access: Book::Title instead of book::Column::Title
            #(#column_constants)*

            /// Get a QuerySet for this model (Django's Model.objects equivalent)
            ///
            /// # Example
            /// ```
            /// use crate::models::Book;
            ///
            /// let books: Vec<Book> = Book::objects(db)
            ///     .filter(Book::Title.contains("Django"))
            ///     .all().await?;
            /// ```
            pub fn objects<C: ::seaorm_django::__internal::ConnectionTrait>(
                db: &C,
            ) -> ::seaorm_django::query::QuerySet<'_, _Entity, C> {
                use ::seaorm_django::query::QueryExt;
                _Entity::objects(db)
            }

            #default_ordering_method

            /// Create the table for this model
            pub async fn create_table<C>(db: &C) -> ::core::result::Result<(), ::seaorm_django::error::ErgormError>
            where
                C: ::seaorm_django::__internal::ConnectionTrait,
            {
                use ::seaorm_django::__internal::{Schema, ConnectionTrait, DbBackend};
                use ::seaorm_django::__internal::sea_query::{MysqlQueryBuilder, PostgresQueryBuilder, SqliteQueryBuilder};

                let backend = db.get_database_backend();
                let schema = Schema::new(backend);
                let stmt = schema.create_table_from_entity(_Entity);

                let sql = match backend {
                    DbBackend::MySql => stmt.to_string(MysqlQueryBuilder),
                    DbBackend::Postgres => stmt.to_string(PostgresQueryBuilder),
                    DbBackend::Sqlite => stmt.to_string(SqliteQueryBuilder),
                    _ => unreachable!("Unsupported database backend"),
                };

                db.execute_unprepared(&sql).await?;

                Ok(())
            }

            /// Drop the table for this model
            pub async fn drop_table<C>(db: &C) -> ::core::result::Result<(), ::seaorm_django::error::ErgormError>
            where
                C: ::seaorm_django::__internal::ConnectionTrait,
            {
                use ::seaorm_django::__internal::{ConnectionTrait, DbBackend};
                use ::seaorm_django::__internal::sea_query::{MysqlQueryBuilder, PostgresQueryBuilder, SqliteQueryBuilder, Table};

                let backend = db.get_database_backend();
                let stmt = Table::drop().table(_Entity).to_owned();

                let sql = match backend {
                    DbBackend::MySql => stmt.to_string(MysqlQueryBuilder),
                    DbBackend::Postgres => stmt.to_string(PostgresQueryBuilder),
                    DbBackend::Sqlite => stmt.to_string(SqliteQueryBuilder),
                    _ => unreachable!("Unsupported database backend"),
                };

                db.execute_unprepared(&sql).await?;

                Ok(())
            }
        }
    })
}

// Helper function to convert snake_case to PascalCase
fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}

/// Generate Default implementation for Model that handles injected relation fields
fn generate_default_impl(
    field_configs: &[(Ident, syn::Type, FieldConfig)],
    foreign_keys: &[(Ident, ForeignKeyConfig)],
) -> TokenStream {
    let mut field_defaults = Vec::new();

    // Generate defaults for all original fields
    for (field_name, field_type, _config) in field_configs {
        field_defaults.push(quote! {
            #field_name: ::core::default::Default::default()
        });
    }

    // Generate defaults for injected relation fields
    for (field_ident, _fk) in foreign_keys {
        let field_name_str = field_ident.to_string();
        let relation_name_str = if field_name_str.ends_with("_id") {
            &field_name_str[..field_name_str.len() - 3]
        } else {
            &field_name_str
        };
        let relation_name = format_ident!("{}", relation_name_str);
        field_defaults.push(quote! {
            #relation_name: ::core::option::Option::None
        });
    }

    quote! {
        impl ::core::default::Default for Model {
            fn default() -> Self {
                Self {
                    #(#field_defaults,)*
                }
            }
        }
    }
}
