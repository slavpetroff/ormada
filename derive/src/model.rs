///! Implementation of the `#[ormada_model]` attribute macro
///!
///! This module provides the core functionality for transforming clean model definitions
///! into SeaORM-compatible code with ergonomic APIs.
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    Attribute, Data, DeriveInput, Expr, Fields, Ident, Lit, Meta, Token,
};

/// Configuration for the `#[ormada_model]` attribute
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

    // Foreign key (Many-to-One)
    foreign_key: Option<ForeignKeyConfig>,

    // One-to-One relationship
    one_to_one: Option<OneToOneConfig>,

    // Many-to-Many relationship
    many_to_many: Option<ManyToManyConfig>,

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
    entity: syn::Path,
    on_delete: Option<Ident>,
    default: Option<Expr>,
}

#[derive(Clone)]
struct OneToOneConfig {
    entity: syn::Path,
    on_delete: Option<Ident>,
}

#[derive(Clone)]
struct ManyToManyConfig {
    entity: syn::Path,
    through: syn::Path,
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
            && !attr.path().is_ident("one_to_one")
            && !attr.path().is_ident("many_to_many")
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
                    // Parse foreign_key(Model, on_delete = Cascade)
                    config.foreign_key = Some(parse_foreign_key(meta_list)?);
                } else if path.is_ident("one_to_one") {
                    // Parse one_to_one(Model, on_delete = Cascade)
                    config.one_to_one = Some(parse_one_to_one(meta_list)?);
                } else if path.is_ident("many_to_many") {
                    // Parse many_to_many(Model, through = JoinModel)
                    config.many_to_many = Some(parse_many_to_many(meta_list)?);
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

fn parse_one_to_one(meta_list: &syn::MetaList) -> syn::Result<OneToOneConfig> {
    let mut model_type = None;
    let mut on_delete = None;

    meta_list.parse_nested_meta(|meta| {
        if model_type.is_none() {
            let path = meta.path.clone();
            let entity_path = if path.segments.len() == 1 {
                let model_name = &path.segments[0].ident;
                let module_name = format_ident!("{}", to_snake_case(&model_name.to_string()));
                syn::parse_quote! { super::super::#module_name::_internal::Entity }
            } else {
                let model_name =
                    &path.segments.last().map(|seg| &seg.ident).expect("Path must have segments");
                let module_name = format_ident!("{}", to_snake_case(&model_name.to_string()));
                syn::parse_quote! { super::super::#module_name::_internal::Entity }
            };
            model_type = Some(entity_path);
        } else if meta.path.is_ident("on_delete") {
            let _: Token![=] = meta.input.parse()?;
            on_delete = Some(meta.input.parse::<Ident>()?);
        }
        Ok(())
    })?;

    Ok(OneToOneConfig {
        entity: model_type.ok_or_else(|| {
            syn::Error::new_spanned(meta_list, "one_to_one requires a Model type")
        })?,
        on_delete,
    })
}

fn parse_many_to_many(meta_list: &syn::MetaList) -> syn::Result<ManyToManyConfig> {
    let mut model_type = None;
    let mut through_type = None;

    meta_list.parse_nested_meta(|meta| {
        if model_type.is_none() {
            let path = meta.path.clone();
            // For M:N, the user provides a path like `Tag` or `super::tag::Tag`
            // The M:N helpers are generated OUTSIDE _internal, so we only need one super
            // From article module, super gets us to models, then tag::_internal::Entity
            let entity_path = if path.segments.len() == 1 {
                // Simple name like `Tag` - assume sibling module
                let model_name = &path.segments[0].ident;
                let module_name = format_ident!("{}", to_snake_case(&model_name.to_string()));
                syn::parse_quote! { super::#module_name::_internal::Entity }
            } else {
                // Full path provided - use it directly but append _internal::Entity
                let mut new_path = path.clone();
                new_path.segments.push(syn::PathSegment {
                    ident: format_ident!("_internal"),
                    arguments: syn::PathArguments::None,
                });
                new_path.segments.push(syn::PathSegment {
                    ident: format_ident!("Entity"),
                    arguments: syn::PathArguments::None,
                });
                new_path
            };
            model_type = Some(entity_path);
        } else if meta.path.is_ident("through") {
            let _: Token![=] = meta.input.parse()?;
            let through_path: syn::Path = meta.input.parse()?;
            // Same logic for through table
            let entity_path = if through_path.segments.len() == 1 {
                let model_name = &through_path.segments[0].ident;
                let module_name = format_ident!("{}", to_snake_case(&model_name.to_string()));
                syn::parse_quote! { super::#module_name::_internal::Entity }
            } else {
                let mut new_path = through_path.clone();
                new_path.segments.push(syn::PathSegment {
                    ident: format_ident!("_internal"),
                    arguments: syn::PathArguments::None,
                });
                new_path.segments.push(syn::PathSegment {
                    ident: format_ident!("Entity"),
                    arguments: syn::PathArguments::None,
                });
                new_path
            };
            through_type = Some(entity_path);
        }
        Ok(())
    })?;

    Ok(ManyToManyConfig {
        entity: model_type.ok_or_else(|| {
            syn::Error::new_spanned(meta_list, "many_to_many requires a Model type")
        })?,
        through: through_type.ok_or_else(|| {
            syn::Error::new_spanned(meta_list, "many_to_many requires 'through = JoinModel'")
        })?,
    })
}

/// Main implementation of the ormada_model attribute macro
pub fn impl_ormada_model(attr: TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
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
                    "ormada_model only supports structs with named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(struct_name, "ormada_model only supports structs"));
        }
    };

    // Parse field configurations and strip our custom attributes
    let mut field_configs = Vec::new();
    let mut has_primary_key = false;
    let mut primary_key_fields = Vec::new();
    let mut foreign_keys: Vec<(Ident, syn::Type, ForeignKeyConfig)> = Vec::new();
    let mut one_to_one_relations: Vec<(Ident, syn::Type, OneToOneConfig)> = Vec::new();
    let mut many_to_many_relations: Vec<(Ident, syn::Type, ManyToManyConfig)> = Vec::new();
    let mut soft_delete_field: Option<Ident> = None;

    // Track which fields to remove (M:N fields are metadata only, not DB columns)
    let mut m2m_field_names = Vec::new();

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
            foreign_keys.push((field_ident.clone(), field_type.clone(), fk.clone()));
        }
        if let Some(ref o2o) = config.one_to_one {
            one_to_one_relations.push((field_ident.clone(), field_type.clone(), o2o.clone()));
            // one_to_one also creates a FK relation for the column
            foreign_keys.push((
                field_ident.clone(),
                field_type.clone(),
                ForeignKeyConfig {
                    entity: o2o.entity.clone(),
                    on_delete: o2o.on_delete.clone(),
                    default: None,
                },
            ));
        }
        if let Some(ref m2m) = config.many_to_many {
            many_to_many_relations.push((field_ident.clone(), field_type.clone(), m2m.clone()));
            m2m_field_names.push(field_ident.to_string());
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

    // Remove M:N fields from the struct - they're metadata only, not DB columns
    fields.named = fields
        .named
        .clone()
        .into_iter()
        .filter(|f| {
            f.ident
                .as_ref()
                .map(|i| !m2m_field_names.contains(&i.to_string()))
                .unwrap_or(true)
        })
        .collect();

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
    // Note: We don't derive Default here because we generate it manually
    // Use the sea_orm derive through our internal module
    // Include Serialize/Deserialize for ModelWithRelations compatibility
    input.attrs.push(syn::parse_quote! {
        #[derive(Clone, Debug, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize, ::ormada::__internal::sea_orm::DeriveEntityModel)]
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

    // NOTE: We NO LONGER inject relation fields into the base Model.
    // Instead, we generate a separate ModelWithRelations struct that wraps Model
    // and adds relation fields. This provides compile-time safety:
    // - Model (from create/update) has no relation fields -> can't accidentally access unloaded relations
    // - ModelWithRelations (from prefetch_related) has relation fields -> safe to access

    // Generate ModelWithRelations struct with relation fields
    let model_with_relations = generate_model_with_relations_struct(&foreign_keys);

    // Generate M:N helper methods
    let m2m_helpers = generate_many_to_many_helpers(&many_to_many_relations);

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
            // Use `#[ormada_model(table = "...", hooks = true)]` to provide custom hooks
            #[::ormada::__internal::async_trait]
            impl ::ormada::hooks::LifecycleHooks for Model {}
        }
    };

    // Generate code with nested module to avoid conflicts
    // This creates the internal SeaORM types and exposes Model as the main interface
    let expanded = quote! {
        // Internal module for SeaORM compatibility - users don't touch this
        // pub(crate) allows other models to reference Entity for relations
        pub(crate) mod _internal {
            use ::serde::{Serialize, Deserialize};
            // Use sea_orm re-exported through ormada to avoid requiring direct dependency
            // All types come from ormada's internal module
            use ::ormada::__internal::sea_orm::entity::prelude::*;
            use ::ormada::__internal::*;
            use ::ormada::prelude::DateTimeWithTimeZone;
            use ::ormada::types::OnDelete;

            // The Model struct with DeriveEntityModel (this generates Entity internally)
            // NOTE: Base Model has NO relation fields - only DB columns
            #input

            // Relation enum
            #relation_enum

            // ActiveModelBehavior implementation
            #entity_impl

            // Ormada entity trait implementation
            #django_entity_impl

            // HasRelation implementations for foreign keys
            #has_relation_impls

            // WithRelationsTrait implementation (required for relations system)
            #with_relations_trait_impl

            // Default implementation for base Model (no relation fields)
            #default_impl

            // ModelWithRelations struct - wraps Model and adds relation fields
            // This is returned by prefetch_related() queries
            #model_with_relations
        }

        // Export Model as the primary type - this is what users work with!
        pub use _internal::Model;

        // Export ModelWithRelations for prefetch_related() queries
        pub use _internal::ModelWithRelations;

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
        impl ::ormada::relations::HasEntityType for Model {
            type __Entity = Entity;
        }

        // LifecycleHooks implementation (auto-generated unless hooks = "custom")
        #lifecycle_hooks_impl

        // Forward OrmadaEntity methods to Entity
        impl Model {
            /// Validate and convert Model to ActiveModel for creation
            ///
            /// This is a convenience method that forwards to the Entity implementation.
            pub fn to_active_model_for_create(model: Self) -> ::core::result::Result<ActiveModel, ::ormada::error::OrmadaError> {
                <Entity as ::ormada::traits::OrmadaEntity>::to_active_model_for_create(model)
            }
        }

        // M:N relationship helper methods
        #m2m_helpers

        // Main export: Author = Model (the data struct users work with)
        pub type #original_name = Model;
    };

    Ok(expanded)
}

/// Extract the inner type from Option<T>, returning T
/// Returns None if the type is not an Option
fn extract_option_inner_type(ty: &syn::Type) -> Option<syn::Type> {
    if let syn::Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Option" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        return Some(inner_ty.clone());
                    }
                }
            }
        }
    }
    None
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

/// Strip ormada-specific attributes from a field, keeping only SeaORM/serde ones
fn strip_django_attributes(field: &mut syn::Field, config: &FieldConfig) {
    // Make field public
    field.vis = syn::Visibility::Public(syn::token::Pub::default());

    // Keep only attributes that SeaORM and serde understand
    let mut new_attrs = Vec::new();

    for attr in &field.attrs {
        // Keep doc comments and other non-ormada attributes
        if !attr.path().is_ident("primary_key")
            && !attr.path().is_ident("foreign_key")
            && !attr.path().is_ident("one_to_one")
            && !attr.path().is_ident("many_to_many")
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
        // Handle auto_increment option for primary key
        if config.auto_increment == Some(false) {
            new_attrs.push(syn::parse_quote! { #[sea_orm(primary_key, auto_increment = false)] });
        } else {
            new_attrs.push(syn::parse_quote! { #[sea_orm(primary_key)] });
        }
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
        // Skip many_to_many fields - they're not actual database columns
        if config.many_to_many.is_some() {
            continue;
        }

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
        #[derive(Clone, Debug, PartialEq, Eq, ::ormada::__internal::sea_orm::DeriveEntityModel, ::serde::Serialize, ::serde::Deserialize, Default)]
        #[sea_orm(table_name = #table_name)]
        pub struct Model {
            #(#fields)*
        }
    })
}

fn generate_column_enum(field_configs: &[(&syn::Field, FieldConfig)]) -> TokenStream {
    let variants: Vec<_> = field_configs
        .iter()
        .filter(|(_, config)| config.many_to_many.is_none()) // Skip M:N fields
        .map(|(field, _)| {
            let name = field.ident.as_ref().unwrap();
            let variant_name = format_ident!("{}", to_pascal_case(&name.to_string()));
            variant_name
        })
        .collect();

    quote! {
        #[derive(Copy, Clone, Debug, ::ormada::__internal::sea_orm::EnumIter, ::ormada::__internal::sea_orm::DeriveColumn)]
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
        #[derive(Copy, Clone, Debug, ::ormada::__internal::sea_orm::EnumIter, ::ormada::__internal::sea_orm::DerivePrimaryKey)]
        pub enum PrimaryKey {
            #(#variants,)*
        }

        impl ::ormada::__internal::PrimaryKeyTrait for PrimaryKey {
            type ValueType = i32; // TODO: Detect actual type
        }
    }
}

fn generate_relation_enum(foreign_keys: &[(Ident, syn::Type, ForeignKeyConfig)]) -> TokenStream {
    if foreign_keys.is_empty() {
        return quote! {
            #[derive(Copy, Clone, Debug, ::ormada::__internal::sea_orm::EnumIter, ::ormada::__internal::sea_orm::DeriveRelation)]
            pub enum Relation {}
        };
    }

    let variants: Vec<_> = foreign_keys
        .iter()
        .map(|(field_name, _fk_field_type, fk)| {
            // Extract meaningful name from the path
            // Path is like: super::super::author::_internal::Entity
            // We want "author" (the module name before _internal)
            let segments: Vec<_> = fk.entity.segments.iter().collect();
            
            // Find the module name by looking for the segment before "_internal"
            let variant_name = segments
                .iter()
                .position(|s| s.ident == "_internal")
                .and_then(|pos| if pos > 0 { Some(&segments[pos - 1].ident) } else { None })
                .unwrap_or_else(|| {
                    // Fallback: use last segment
                    &segments.last().unwrap().ident
                });
            
            let variant_ident = format_ident!("{}", to_pascal_case(&variant_name.to_string()));

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
                #variant_ident
            }
        })
        .collect();

    quote! {
        #[derive(Copy, Clone, Debug, ::ormada::__internal::sea_orm::EnumIter, ::ormada::__internal::sea_orm::DeriveRelation)]
        pub enum Relation {
            #(#variants,)*
        }
    }
}

fn generate_entity_impl() -> TokenStream {
    quote! {
        impl ::ormada::__internal::ActiveModelBehavior for ActiveModel {}
    }
}

/// Generate helper methods for M:N relationships defined with #[many_to_many(Model, through = JoinModel)]
fn generate_many_to_many_helpers(
    m2m_relations: &[(Ident, syn::Type, ManyToManyConfig)],
) -> TokenStream {
    if m2m_relations.is_empty() {
        return quote! {};
    }

    let methods: Vec<_> = m2m_relations
        .iter()
        .map(|(field_name, _field_type, config)| {
            // Extract model names from paths
            // config.entity is like: super::tag::_internal::Entity
            // config.through is like: super::article_tag::_internal::Entity
            let entity_segments: Vec<_> = config.entity.segments.iter().collect();
            let through_segments: Vec<_> = config.through.segments.iter().collect();

            // Find the module name before _internal for the related model
            let related_module = entity_segments
                .iter()
                .position(|s| s.ident == "_internal")
                .and_then(|pos| {
                    if pos > 0 {
                        Some(&entity_segments[pos - 1].ident)
                    } else {
                        None
                    }
                })
                .map(|i| i.to_string())
                .unwrap_or_else(|| "related".to_string());

            // Find the module name before _internal for the through table
            let through_module = through_segments
                .iter()
                .position(|s| s.ident == "_internal")
                .and_then(|pos| {
                    if pos > 0 {
                        Some(&through_segments[pos - 1].ident)
                    } else {
                        None
                    }
                })
                .map(|i| i.to_string())
                .unwrap_or_else(|| "through".to_string());

            // Generate method name
            let get_method_name = format_ident!("get_{}", field_name);

            // Build paths to the Model types (not Entity)
            // From super::tag::_internal::Entity, we want super::tag::Model
            let related_model_path = {
                let mut segments: Vec<_> = config.entity.segments.iter().cloned().collect();
                // Remove _internal and Entity, keep up to module name
                while segments.last().map(|s| s.ident == "Entity" || s.ident == "_internal").unwrap_or(false) {
                    segments.pop();
                }
                // Add Model
                segments.push(syn::PathSegment {
                    ident: format_ident!("Model"),
                    arguments: syn::PathArguments::None,
                });
                let mut path = syn::Path { leading_colon: None, segments: syn::punctuated::Punctuated::new() };
                for seg in segments {
                    path.segments.push(seg);
                }
                path
            };

            let through_model_path = {
                let mut segments: Vec<_> = config.through.segments.iter().cloned().collect();
                while segments.last().map(|s| s.ident == "Entity" || s.ident == "_internal").unwrap_or(false) {
                    segments.pop();
                }
                segments.push(syn::PathSegment {
                    ident: format_ident!("Model"),
                    arguments: syn::PathArguments::None,
                });
                let mut path = syn::Path { leading_colon: None, segments: syn::punctuated::Punctuated::new() };
                for seg in segments {
                    path.segments.push(seg);
                }
                path
            };

            // Column names for the through table (PascalCase)
            // The through table has article_id and tag_id columns
            // We need ArticleId (for self) and TagId (for related)
            let self_column_name = format_ident!("{}Id", to_pascal_case(&through_module.trim_end_matches("_tag").trim_end_matches("_article")));
            let related_column_name = format_ident!("{}Id", to_pascal_case(&related_module));

            // Field names for accessing the through record (snake_case)
            let related_fk_field = format_ident!("{}_id", to_snake_case(&related_module));

            quote! {
                /// Get all related models through the M:N relationship
                ///
                /// This method queries the through table and returns all related models.
                /// Uses Ormada's QuerySet API for type-safe queries.
                pub async fn #get_method_name<C: ::ormada::db::ConnectionTrait>(
                    &self,
                    db: &C,
                ) -> ::core::result::Result<Vec<#related_model_path>, ::ormada::error::OrmadaError> {
                    use ::ormada::prelude::*;

                    // Query through table filtered by self.id
                    let through_records = #through_model_path::objects(db)
                        .filter(#through_model_path::#self_column_name.eq(self.id))
                        .all()
                        .await?;

                    if through_records.is_empty() {
                        return Ok(Vec::new());
                    }

                    // Extract related IDs
                    let related_ids: Vec<_> = through_records
                        .iter()
                        .map(|r| r.#related_fk_field)
                        .collect();

                    // Query related models
                    let related = #related_model_path::objects(db)
                        .filter(#related_model_path::Id.is_in(related_ids))
                        .all()
                        .await?;

                    Ok(related)
                }
            }
        })
        .collect();

    quote! {
        impl Model {
            #(#methods)*
        }
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
        // Skip M:N fields - they're not actual DB columns
        if config.many_to_many.is_some() {
            continue;
        }

        // Generate validation code
        let field_name_str = field_name.to_string();

        // Foreign key validation for non-nullable FKs
        // Check that FK value is not the default value which would likely cause DB constraint violation
        // This works for all types that implement Default + PartialEq:
        // - i8, i16, i32, i64: default is 0
        // - u8, u16, u32, u64: default is 0
        // - String: default is ""
        // - Uuid: default is nil UUID (00000000-0000-0000-0000-000000000000)
        if let Some(ref _fk) = config.foreign_key {
            let type_str = quote!(#field_type).to_string();
            let is_nullable_fk = type_str.contains("Option");

            if !is_nullable_fk {
                // Non-nullable FK: validate that value is not the default
                // Use the field type explicitly to avoid type inference issues
                validations.push(quote! {
                    if model.#field_name == <#field_type as ::core::default::Default>::default() {
                        return ::core::result::Result::Err(
                            ::ormada::error::OrmadaError::validation_error(
                                #table_name,
                                #field_name_str,
                                "foreign key cannot be the default value - did you forget to set this field? Using Default::default() on models with foreign keys will leave FK fields uninitialized."
                            )
                        );
                    }
                });
            }
        }

        // String length validations
        if config.max_length.is_some() || config.min_length.is_some() {
            let type_str = quote!(#field_type).to_string();
            if type_str.contains("String") {
                if let Some(max) = config.max_length {
                    validations.push(quote! {
                        if model.#field_name.len() > #max {
                            return ::core::result::Result::Err(
                                ::ormada::error::OrmadaError::validation_error(
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
                                ::ormada::error::OrmadaError::validation_error(
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
                            ::ormada::error::OrmadaError::validation_error(
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
                            ::ormada::error::OrmadaError::validation_error(
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
                #field_name: ::ormada::__internal::Set(now)
            });
        } else if config.is_primary_key {
            // For auto_increment = false PKs (like UUID), we need to Set the value
            // For auto_increment PKs, we use NotSet to let the DB generate the value
            if config.auto_increment == Some(false) {
                create_assignments.push(quote! {
                    #field_name: ::ormada::__internal::Set(model.#field_name.clone())
                });
            } else {
                create_assignments.push(quote! {
                    #field_name: ::ormada::__internal::NotSet
                });
            }
        } else if let Some(ref _fk) = config.foreign_key {
            create_assignments.push(quote! {
                #field_name: ::ormada::__internal::Set(model.#field_name)
            });
        } else {
            // Regular field
            create_assignments.push(quote! {
                #field_name: ::ormada::__internal::Set(model.#field_name)
            });
        }
    }

    // Generate soft_delete implementation using enum
    let soft_delete_impl = if let Some(field) = soft_delete_field {
        let field_str = field.to_string();
        quote! {
            fn soft_delete() -> ::ormada::traits::SoftDeleteConfig {
                ::ormada::traits::SoftDeleteConfig::Enabled { column: #field_str }
            }
        }
    } else {
        quote! {}
    };

    Ok(quote! {
        impl ::ormada::traits::OrmadaEntity for Entity {
            fn to_active_model_for_create(model: Model) -> ::core::result::Result<ActiveModel, ::ormada::error::OrmadaError> {
                // Validation logic
                #(#validations)*

                let now = ::ormada::__internal::Utc::now().fixed_offset();
                ::core::result::Result::Ok(ActiveModel {
                    #(#create_assignments,)*
                })
            }

            async fn save_model<'a, C: ::ormada::__internal::ConnectionTrait>(
                db: &'a C,
                model: Model,
            ) -> ::core::result::Result<Model, ::ormada::error::OrmadaError> {
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
fn generate_with_relations_trait(
    _foreign_keys: &[(Ident, syn::Type, ForeignKeyConfig)],
) -> TokenStream {
    // ModelWithRelations is now a separate struct that wraps Model
    // from_model_and_relations converts Model -> ModelWithRelations
    quote! {
        impl ::ormada::traits::WithRelationsTrait for Entity {
            type Model = Model;
            type ModelWithRelations = ModelWithRelations;

            fn from_model_and_relations<R>(
                model: Self::Model,
                _relations: &R,
            ) -> Self::ModelWithRelations {
                // Convert Model to ModelWithRelations using From impl
                ::core::convert::From::from(model)
            }
        }
    }
}

fn generate_has_relation_impls(
    foreign_keys: &[(Ident, syn::Type, ForeignKeyConfig)],
) -> TokenStream {
    let impls: Vec<_> = foreign_keys
        .iter()
        .map(|(field_name, fk_field_type, fk)| {
            let entity = &fk.entity;
            let relation_name_str = if field_name.to_string().ends_with("_id") {
                &field_name.to_string()[..field_name.to_string().len() - 3]
            } else {
                &field_name.to_string()
            };
            let relation_name = format_ident!("{}", relation_name_str);

            // Check if FK field is nullable (Option<T>)
            let fk_type_str = quote!(#fk_field_type).to_string();
            let is_nullable_fk = fk_type_str.contains("Option");

            // Generate set_related based on FK nullability
            // NOTE: set_related now works on ModelWithRelations, not Model
            let set_related_impl = if is_nullable_fk {
                // Nullable FK: relation field is Option<Model>
                quote! {
                    fn set_related(model: &mut <Self as ::ormada::traits::WithRelationsTrait>::ModelWithRelations, related: ::core::option::Option<<#entity as ::ormada::__internal::EntityTrait>::Model>) {
                        model.#relation_name = related;
                    }
                }
            } else {
                // Non-nullable FK: relation field is Model directly
                // If related is None, we use Default (this shouldn't happen with proper prefetch)
                quote! {
                    fn set_related(model: &mut <Self as ::ormada::traits::WithRelationsTrait>::ModelWithRelations, related: ::core::option::Option<<#entity as ::ormada::__internal::EntityTrait>::Model>) {
                        if let Some(r) = related {
                            model.#relation_name = r;
                        }
                    }
                }
            };

            // Generate get_foreign_key and load_related based on FK nullability
            let (get_fk_impl, load_related_impl) = if is_nullable_fk {
                // Nullable FK: need to handle Option<T> where T can be i32, i64, String, etc.
                (
                    quote! {
                        fn get_foreign_key(model: &<Self as ::ormada::__internal::EntityTrait>::Model) -> Self::RelatedPK {
                            model.#field_name.clone().unwrap_or_default()
                        }
                    },
                    quote! {
                        async fn load_related<C: ::ormada::__internal::ConnectionTrait>(
                            models: &[<Self as ::ormada::__internal::EntityTrait>::Model],
                            db: &C,
                        ) -> ::core::result::Result<
                            ::ormada::prelude::FxHashMap<Self::RelatedPK, <#entity as ::ormada::__internal::EntityTrait>::Model>,
                            ::ormada::error::OrmadaError
                        > {
                            use ::ormada::__internal::{EntityTrait, QueryFilter, ColumnTrait, Iterable};

                            // For nullable FKs, filter out None values
                            let fk_values: ::std::vec::Vec<Self::RelatedPK> = models
                                .iter()
                                .filter_map(|m| m.#field_name)
                                .collect();

                            if fk_values.is_empty() {
                                return ::core::result::Result::Ok(::ormada::prelude::FxHashMap::default());
                            }

                            let pk_cols: ::std::vec::Vec<_> = <#entity as ::ormada::__internal::EntityTrait>::PrimaryKey::iter()
                                .map(|pk| pk.into_column())
                                .collect();
                            let id_column = pk_cols[0];

                            let related_models = <#entity as ::ormada::__internal::EntityTrait>::find()
                                .filter(id_column.is_in(fk_values))
                                .all(db)
                                .await?;

                            let mut map = ::ormada::prelude::FxHashMap::default();
                            for model in related_models {
                                let key = model.id;
                                map.insert(key, model);
                            }

                            ::core::result::Result::Ok(map)
                        }
                    }
                )
            } else {
                // Non-nullable FK: direct i32 access
                (
                    quote! {
                        fn get_foreign_key(model: &<Self as ::ormada::__internal::EntityTrait>::Model) -> Self::RelatedPK {
                            model.#field_name
                        }
                    },
                    quote! {
                        async fn load_related<C: ::ormada::__internal::ConnectionTrait>(
                            models: &[<Self as ::ormada::__internal::EntityTrait>::Model],
                            db: &C,
                        ) -> ::core::result::Result<
                            ::ormada::prelude::FxHashMap<Self::RelatedPK, <#entity as ::ormada::__internal::EntityTrait>::Model>,
                            ::ormada::error::OrmadaError
                        > {
                            use ::ormada::__internal::{EntityTrait, QueryFilter, ColumnTrait, Iterable};

                            let fk_values: ::std::vec::Vec<Self::RelatedPK> = models
                                .iter()
                                .map(|m| m.#field_name)
                                .collect();

                            if fk_values.is_empty() {
                                return ::core::result::Result::Ok(::ormada::prelude::FxHashMap::default());
                            }

                            let pk_cols: ::std::vec::Vec<_> = <#entity as ::ormada::__internal::EntityTrait>::PrimaryKey::iter()
                                .map(|pk| pk.into_column())
                                .collect();
                            let id_column = pk_cols[0];

                            let related_models = <#entity as ::ormada::__internal::EntityTrait>::find()
                                .filter(id_column.is_in(fk_values))
                                .all(db)
                                .await?;

                            let mut map = ::ormada::prelude::FxHashMap::default();
                            for model in related_models {
                                let key = model.id;
                                map.insert(key, model);
                            }

                            ::core::result::Result::Ok(map)
                        }
                    }
                )
            };

            // Extract the inner type for nullable FKs (Option<T> -> T)
            let related_pk_type = if is_nullable_fk {
                // For Option<T>, extract T
                // fk_field_type is like "Option < i32 >" or "Option<i64>"
                // We need to extract the inner type
                let inner_type = extract_option_inner_type(fk_field_type);
                inner_type.unwrap_or_else(|| fk_field_type.clone())
            } else {
                fk_field_type.clone()
            };

            quote! {
                impl ::ormada::relations::HasRelation<#entity> for Entity {
                    type RelatedPK = #related_pk_type;

                    #get_fk_impl

                    #set_related_impl

                    #load_related_impl
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
        // Skip M:N fields
        if config.many_to_many.is_some() {
            return None;
        }
        if config.auto_now {
            Some(quote! {
                active_model.#ident = ::ormada::__internal::Set(::ormada::__internal::Utc::now().fixed_offset());
            })
        } else {
            None
        }
    });

    // Force all fields to Set to ensure they are updated
    // ActiveModel::from() sets them to Unchanged, which causes save() to skip updating them
    let force_set_updates = field_configs.iter().filter_map(|(ident, _, config)| {
        // Skip M:N fields - they're not actual DB columns
        if config.many_to_many.is_some() {
            return None;
        }
        if !config.is_primary_key && !config.auto_now {
            Some(quote! {
                active_model.#ident = ::ormada::__internal::Set(active_model.#ident.unwrap());
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
            pub async fn save<'a, C: ::ormada::__internal::ConnectionTrait>(
                mut self,
                db: &'a C,
            ) -> ::core::result::Result<Self, ::ormada::error::OrmadaError> {
                use ::ormada::hooks::LifecycleHooks;
                use ::ormada::__internal::ActiveModelTrait;
                use ::ormada::__internal::TryIntoModel;

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
                pub async fn delete<C: ::ormada::__internal::ConnectionTrait>(
                    mut self,
                    db: &C,
                ) -> ::core::result::Result<Self, ::ormada::error::OrmadaError> {
                    use ::ormada::__internal::{ActiveModelTrait, Set, ActiveValue};
                    use ::ormada::hooks::LifecycleHooks;

                    <Self as LifecycleHooks>::before_delete(&self).await?;

                    // Convert to ActiveModel and set deleted_at
                    let mut active = ActiveModel::from(self);
                    active.#field_name = Set(::core::option::Option::Some(::ormada::__internal::Utc::now().fixed_offset()));

                    // Update in database
                    let updated = active.update(db).await?;

                    <Self as LifecycleHooks>::after_delete(&updated).await?;

                    ::core::result::Result::Ok(updated)
                }

                /// Permanently delete this record from the database (hard delete).
                ///
                /// This cannot be undone. Use `.delete()` for soft delete instead.
                pub async fn force_delete<C: ::ormada::__internal::ConnectionTrait>(
                    mut self,
                    db: &C,
                ) -> ::core::result::Result<(), ::ormada::error::OrmadaError> {
                    use ::ormada::__internal::ActiveModelTrait;
                    use ::ormada::hooks::LifecycleHooks;

                    <Self as LifecycleHooks>::before_delete(&self).await?;

                    let active = ActiveModel::from(self.clone());
                    active.delete(db).await?;

                    <Self as LifecycleHooks>::after_delete(&self).await?;

                    ::core::result::Result::Ok(())
                }

                /// Restore a soft-deleted record (set deleted_at to NULL).
                ///
                /// Makes the record visible in queries again.
                pub async fn restore<C: ::ormada::__internal::ConnectionTrait>(
                    mut self,
                    db: &C,
                ) -> ::core::result::Result<Self, ::ormada::error::OrmadaError> {
                    use ::ormada::__internal::{ActiveModelTrait, Set, ActiveValue};
                    use ::ormada::hooks::LifecycleHooks;

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
                pub async fn delete<C: ::ormada::__internal::ConnectionTrait>(
                    mut self,
                    db: &C,
                ) -> ::core::result::Result<(), ::ormada::error::OrmadaError> {
                    use ::ormada::__internal::ActiveModelTrait;
                    use ::ormada::hooks::LifecycleHooks;

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
                /// Apply default ordering (from #[ormada_model(ordering = "...")])
                pub fn default_ordering<C: ::ormada::__internal::ConnectionTrait>(
                    db: &C,
                ) -> ::ormada::query::QuerySet<'_, _Entity, C> {
                    use ::ormada::query::QueryExt;
                    _Entity::objects(db).order_by_desc(Self::#column_ident)
                }
            }
        } else {
            quote! {
                /// Apply default ordering (from #[ormada_model(ordering = "...")])
                pub fn default_ordering<C: ::ormada::__internal::ConnectionTrait>(
                    db: &C,
                ) -> ::ormada::query::QuerySet<'_, _Entity, C> {
                    use ::ormada::query::QueryExt;
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

            /// Get a QuerySet for this model (Ormada's Model.objects equivalent)
            ///
            /// # Example
            /// ```
            /// use crate::models::Book;
            ///
            /// let books: Vec<Book> = Book::objects(db)
            ///     .filter(Book::Title.contains("Ormada"))
            ///     .all().await?;
            /// ```
            pub fn objects<C: ::ormada::__internal::ConnectionTrait>(
                db: &C,
            ) -> ::ormada::query::QuerySet<'_, _Entity, C> {
                use ::ormada::query::QueryExt;
                _Entity::objects(db)
            }

            #default_ordering_method

            /// Create the table for this model
            pub async fn create_table<C>(db: &C) -> ::core::result::Result<(), ::ormada::error::OrmadaError>
            where
                C: ::ormada::__internal::ConnectionTrait,
            {
                use ::ormada::__internal::{Schema, ConnectionTrait, DbBackend};
                use ::ormada::__internal::sea_query::{MysqlQueryBuilder, PostgresQueryBuilder, SqliteQueryBuilder};

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
            pub async fn drop_table<C>(db: &C) -> ::core::result::Result<(), ::ormada::error::OrmadaError>
            where
                C: ::ormada::__internal::ConnectionTrait,
            {
                use ::ormada::__internal::{ConnectionTrait, DbBackend};
                use ::ormada::__internal::sea_query::{MysqlQueryBuilder, PostgresQueryBuilder, SqliteQueryBuilder, Table};

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

/// Generate Default implementation for Model (base model without relation fields)
///
/// Default is always generated. For models with required FK fields, the FK will
/// default to 0 which will fail at the database level if not overridden.
/// Users should always explicitly provide FK values:
///
/// ```ignore
/// Book {
///     author_id: author.id,  // Required - must be provided
///     title: "My Book".to_string(),
///     price: 1999,
///     published: true,
///     ..Default::default()   // Fills id, created_at, updated_at
/// }
/// ```
fn generate_default_impl(
    field_configs: &[(Ident, syn::Type, FieldConfig)],
    _foreign_keys: &[(Ident, syn::Type, ForeignKeyConfig)],
) -> TokenStream {
    let mut field_defaults = Vec::new();

    // Generate defaults for all original fields (no relation fields on base Model)
    // Skip M:N fields - they're not actual DB columns
    for (field_name, _field_type, config) in field_configs {
        if config.many_to_many.is_some() {
            continue;
        }
        field_defaults.push(quote! {
            #field_name: ::core::default::Default::default()
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

/// Generate ModelWithRelations struct that wraps Model and adds relation fields
///
/// This struct is returned by prefetch_related() queries and provides type-safe
/// access to loaded relations. The base Model (from create/update) does NOT have
/// relation fields, preventing accidental access to unloaded relations.
fn generate_model_with_relations_struct(
    foreign_keys: &[(Ident, syn::Type, ForeignKeyConfig)],
) -> TokenStream {
    if foreign_keys.is_empty() {
        // No relations - ModelWithRelations is a newtype wrapper around Model
        // This ensures Deref<Target = Model> is always available
        return quote! {
            /// Model with loaded relations (wrapper for Model when no relations exist)
            #[derive(Debug, Clone, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
            #[serde(transparent)]
            pub struct ModelWithRelations(pub Model);

            impl ::core::default::Default for ModelWithRelations {
                fn default() -> Self {
                    Self(::core::default::Default::default())
                }
            }

            impl ::core::ops::Deref for ModelWithRelations {
                type Target = Model;

                fn deref(&self) -> &Self::Target {
                    &self.0
                }
            }

            impl ::core::ops::DerefMut for ModelWithRelations {
                fn deref_mut(&mut self) -> &mut Self::Target {
                    &mut self.0
                }
            }

            impl ::core::convert::From<Model> for ModelWithRelations {
                fn from(model: Model) -> Self {
                    Self(model)
                }
            }
        };
    }

    // Generate relation fields for ModelWithRelations
    let relation_fields: Vec<_> = foreign_keys
        .iter()
        .map(|(field_ident, fk_field_type, fk)| {
            let field_name_str = field_ident.to_string();
            let relation_name_str = if field_name_str.ends_with("_id") {
                &field_name_str[..field_name_str.len() - 3]
            } else {
                &field_name_str
            };
            let relation_name = format_ident!("{}", relation_name_str);
            let relation_type = &fk.entity;

            // Check if FK field is nullable (Option<T>)
            let fk_type_str = quote!(#fk_field_type).to_string();
            let is_nullable_fk = fk_type_str.contains("Option");

            if is_nullable_fk {
                quote! {
                    pub #relation_name: ::core::option::Option<<#relation_type as ::ormada::__internal::EntityTrait>::Model>
                }
            } else {
                quote! {
                    pub #relation_name: <#relation_type as ::ormada::__internal::EntityTrait>::Model
                }
            }
        })
        .collect();

    // Generate Default for relation fields
    let relation_defaults: Vec<_> = foreign_keys
        .iter()
        .map(|(field_ident, fk_field_type, _fk)| {
            let field_name_str = field_ident.to_string();
            let relation_name_str = if field_name_str.ends_with("_id") {
                &field_name_str[..field_name_str.len() - 3]
            } else {
                &field_name_str
            };
            let relation_name = format_ident!("{}", relation_name_str);

            let fk_type_str = quote!(#fk_field_type).to_string();
            let is_nullable_fk = fk_type_str.contains("Option");

            if is_nullable_fk {
                quote! { #relation_name: ::core::option::Option::None }
            } else {
                quote! { #relation_name: ::core::default::Default::default() }
            }
        })
        .collect();

    quote! {
        /// Model with loaded relations
        ///
        /// This struct is returned by `prefetch_related()` and `select_related()` queries.
        /// It contains the base model fields plus loaded relation fields.
        ///
        /// The base `Model` type (returned by `create()`, `update()`, queries without prefetch)
        /// does NOT have relation fields, providing compile-time safety against accessing
        /// unloaded relations.
        #[derive(Debug, Clone, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
        pub struct ModelWithRelations {
            /// The base model with all database fields
            #[serde(flatten)]
            pub inner: Model,
            /// Loaded relation fields
            #(#relation_fields,)*
        }

        impl ::core::default::Default for ModelWithRelations {
            fn default() -> Self {
                Self {
                    inner: ::core::default::Default::default(),
                    #(#relation_defaults,)*
                }
            }
        }

        impl ::core::ops::Deref for ModelWithRelations {
            type Target = Model;

            fn deref(&self) -> &Self::Target {
                &self.inner
            }
        }

        impl ::core::ops::DerefMut for ModelWithRelations {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.inner
            }
        }

        impl ::core::convert::From<Model> for ModelWithRelations {
            fn from(model: Model) -> Self {
                Self {
                    inner: model,
                    #(#relation_defaults,)*
                }
            }
        }
    }
}
