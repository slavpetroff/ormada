///! Implementation of the `#[django_model]` attribute macro
///!
///! This module provides the core functionality for transforming clean model definitions
///! into SeaORM-compatible code with Django-like ergonomics.
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    Attribute, Data, DeriveInput, Expr, Fields, Ident, Lit, Meta, Token,
};

/// Configuration for the `#[django_model]` attribute
#[derive(Debug, Clone)]
struct ModelConfig {
    table_name: String,
    composite_indexes: Vec<CompositeIndex>,
}

/// Composite index definition
#[derive(Debug, Clone)]
struct CompositeIndex {
    fields: Vec<String>,
    name: Option<String>,
}

/// Field-level attribute configuration
#[derive(Debug, Clone, Default)]
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

    // Serialization
    skip_serializing: bool,
    skip_deserializing: bool,
}

#[derive(Debug, Clone)]
struct ForeignKeyConfig {
    entity: syn::Path,  // Can be Author or super::author::Entity
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
    let mut entity = None;
    let mut on_delete = None;
    let mut default = None;

    meta_list.parse_nested_meta(|meta| {
        if entity.is_none() {
            // First positional argument is the entity (can be path or identifier)
            // Accept both: Author or super::author::Entity
            entity = Some(meta.path.clone());
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
        entity: entity
            .ok_or_else(|| syn::Error::new_spanned(meta_list, "foreign_key requires an entity"))?,
        on_delete,
        default,
    })
}

/// Main implementation of the django_model attribute macro
pub fn impl_django_model(
    attr: TokenStream,
    input: TokenStream,
) -> syn::Result<TokenStream> {
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
            return Err(syn::Error::new_spanned(
                struct_name,
                "django_model only supports structs",
            ));
        }
    };

    // Parse field configurations and strip our custom attributes
    let mut field_configs = Vec::new();
    let mut has_primary_key = false;
    let mut primary_key_fields = Vec::new();
    let mut foreign_keys = Vec::new();

    for field in fields.named.iter_mut() {
        let config = parse_field_attributes(&field.attrs)?;
        
        // Validation
        if config.is_primary_key {
            has_primary_key = true;
            primary_key_fields.push(field.ident.as_ref().unwrap().clone());
        }
        if let Some(ref fk) = config.foreign_key {
            foreign_keys.push((field.ident.as_ref().unwrap().clone(), fk.clone()));
        }
        
        // Store config before stripping attributes
        field_configs.push((field.ident.as_ref().unwrap().clone(), field.ty.clone(), config.clone()));
        
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
    input.attrs.push(syn::parse_quote! {
        #[derive(Clone, Debug, PartialEq, Eq, ::sea_orm::entity::prelude::DeriveEntityModel, Default)]
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

    // Generate additional components
    let relation_enum = generate_relation_enum(&foreign_keys);
    let entity_impl = generate_entity_impl();
    let django_entity_impl = generate_django_entity_impl(&field_configs)?;
    let has_relation_impls = generate_has_relation_impls(&foreign_keys);
    let model_save_impl = generate_model_save_impl(&field_configs)?;
    let model_convenience_impl = generate_model_convenience_methods(&input)?;

    // Generate code with nested module to avoid conflicts
    // Creates: author::author::Entity, but re-exports as author::Entity
    let expanded = quote! {
        pub mod #module_name {
            // Import only what we need explicitly - no super::* to avoid conflicts
            use ::serde::{Serialize, Deserialize};
            use ::sea_orm::entity::prelude::{
                DeriveEntityModel, EnumIter, Related, RelationDef, RelationTrait,
                ActiveModelBehavior, EntityTrait, PrimaryKeyTrait, ColumnTrait,
                DeriveColumn, DerivePrimaryKey, DeriveRelation,
            };
            use ::sea_orm::PrimaryKeyToColumn; // For HasRelation implementation
            use ::seaorm_django::prelude::DateTimeWithTimeZone; // For datetime fields
            use ::seaorm_django::types::OnDelete; // For foreign key cascades
            
            // The Model struct with DeriveEntityModel
            #input
            
            // Relation enum
            #relation_enum
            
            // ActiveModelBehavior implementation
            #entity_impl
            
            // Django entity trait implementation
            #django_entity_impl
            
            // HasRelation implementations for foreign keys
            #has_relation_impls
            
            // Model.save() method
            #model_save_impl
            
            // Convenience methods on Model
            #model_convenience_impl
        }
        
        // Re-export all items at parent level for cleaner imports
        // Enables: author::Entity instead of author::author::Entity
        pub use #module_name::{Entity, Model, ActiveModel, Column, PrimaryKey, Relation};
        
        // Main export: Book = Entity (for .objects() calls), with Column constants attached
        // The actual data type is still Model, but Entity is what you query with
        pub type #original_name = Entity;
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
        #[derive(Clone, Debug, PartialEq, Eq, ::sea_orm::entity::prelude::DeriveEntityModel, ::serde::Serialize, ::serde::Deserialize, Default)]
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
        #[derive(Copy, Clone, Debug, ::sea_orm::EnumIter, ::sea_orm::DeriveColumn)]
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
        #[derive(Copy, Clone, Debug, ::sea_orm::EnumIter, ::sea_orm::DerivePrimaryKey)]
        pub enum PrimaryKey {
            #(#variants,)*
        }

        impl ::sea_orm::PrimaryKeyTrait for PrimaryKey {
            type ValueType = i32; // TODO: Detect actual type
        }
    }
}

fn generate_relation_enum(foreign_keys: &[(Ident, ForeignKeyConfig)]) -> TokenStream {
    if foreign_keys.is_empty() {
        return quote! {
            #[derive(Copy, Clone, Debug, ::sea_orm::EnumIter, ::sea_orm::DeriveRelation)]
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
            let variant_name = if segments.len() >= 2 && segments.last().unwrap().ident == "Entity" {
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
                proc_macro2::Span::call_site()
            );
            let to_path = syn::LitStr::new(
                &to_path_str,
                proc_macro2::Span::call_site()
            );
            
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
        #[derive(Copy, Clone, Debug, ::sea_orm::EnumIter, ::sea_orm::DeriveRelation)]
        pub enum Relation {
            #(#variants,)*
        }
    }
}

fn generate_entity_impl() -> TokenStream {
    quote! {
        impl ::sea_orm::ActiveModelBehavior for ActiveModel {}
    }
}

fn generate_django_entity_impl(
    field_configs: &[(Ident, syn::Type, FieldConfig)],
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
                                ::seaorm_django::error::DjangoOrmError::ValidationError(
                                    ::std::format!("Field '{}' exceeds max_length of {}", #field_name_str, #max)
                                )
                            );
                        }
                    });
                }
                if let Some(min) = config.min_length {
                    validations.push(quote! {
                        if model.#field_name.len() < #min {
                            return ::core::result::Result::Err(
                                ::seaorm_django::error::DjangoOrmError::ValidationError(
                                    ::std::format!("Field '{}' is shorter than min_length of {}", #field_name_str, #min)
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
                            ::seaorm_django::error::DjangoOrmError::ValidationError(
                                ::std::format!("Field '{}' value {} is less than minimum {}", #field_name_str, model.#field_name, #min)
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
                            ::seaorm_django::error::DjangoOrmError::ValidationError(
                                ::std::format!("Field '{}' value {} exceeds maximum {}", #field_name_str, model.#field_name, #max)
                            )
                        );
                    }
                });
            }
        }
        
        // Generate field assignment
        if config.auto_now_add {
            create_assignments.push(quote! {
                #field_name: ::sea_orm::Set(now)
            });
        } else if config.is_primary_key {
            create_assignments.push(quote! {
                #field_name: ::sea_orm::NotSet
            });
        } else {
            create_assignments.push(quote! {
                #field_name: ::sea_orm::Set(model.#field_name)
            });
        }
    }

    Ok(quote! {
        impl ::seaorm_django::traits::DjangoEntity for Entity {
            fn to_active_model_for_create(model: Model) -> ::core::result::Result<ActiveModel, ::seaorm_django::error::DjangoOrmError> {
                // Validation logic
                #(#validations)*
                
                let now = ::chrono::Utc::now().fixed_offset();
                ::core::result::Result::Ok(ActiveModel {
                    #(#create_assignments,)*
                })
            }

            async fn save_model<'a, C: ::sea_orm::ConnectionTrait>(
                db: &'a C,
                model: Model,
            ) -> ::core::result::Result<Model, ::seaorm_django::error::DjangoOrmError> {
                model.save(db).await
            }
        }
    })
}

fn generate_has_relation_impls(foreign_keys: &[(Ident, ForeignKeyConfig)]) -> TokenStream {
    let impls: Vec<_> = foreign_keys
        .iter()
        .map(|(field_name, fk)| {
            let entity = &fk.entity;
            quote! {
                impl ::seaorm_django::relations::HasRelation<#entity> for Entity {
                    type RelatedPK = i32; // TODO: Detect actual type
                    
                    fn get_foreign_key(model: &Self::Model) -> Self::RelatedPK {
                        model.#field_name
                    }

                    async fn load_related<C: ::sea_orm::ConnectionTrait>(
                        models: &[Self::Model],
                        db: &C,
                    ) -> ::core::result::Result<
                        ::seaorm_django::prelude::FxHashMap<Self::RelatedPK, <#entity as ::sea_orm::EntityTrait>::Model>,
                        ::seaorm_django::error::DjangoOrmError
                    > {
                        use ::sea_orm::{EntityTrait, QueryFilter, ColumnTrait, Iterable};

                        let fk_values: ::std::vec::Vec<Self::RelatedPK> = models
                            .iter()
                            .map(|m| m.#field_name)
                            .collect();

                        if fk_values.is_empty() {
                            return ::core::result::Result::Ok(::seaorm_django::prelude::FxHashMap::default());
                        }

                        let pk_cols: ::std::vec::Vec<_> = <#entity as ::sea_orm::EntityTrait>::PrimaryKey::iter()
                            .map(|pk| pk.into_column())
                            .collect();
                        let id_column = pk_cols[0];

                        let related_models = <#entity as ::sea_orm::EntityTrait>::find()
                            .filter(id_column.is_in(fk_values))
                            .all(db)
                            .await?;

                        ::core::result::Result::Ok(
                            ::seaorm_django::prelude::FxHashMap::from_iter(
                                related_models
                                    .into_iter()
                                    .map(|m| (m.id, m))
                            )
                        )
                    }
                }
            }
        })
        .collect();

    quote! {
        #(#impls)*
    }
}

fn generate_model_save_impl(
    field_configs: &[(Ident, syn::Type, FieldConfig)],
) -> syn::Result<TokenStream> {
    let mut save_assignments = Vec::new();
    let mut auto_now_updates = Vec::new();

    for (field_name, _field_type, config) in field_configs {
        if config.auto_now {
            save_assignments.push(quote! {
                #field_name: ::sea_orm::Set(model.#field_name)
            });
            auto_now_updates.push(quote! {
                active_model.#field_name = ::sea_orm::Set(now);
            });
        } else {
            save_assignments.push(quote! {
                #field_name: ::sea_orm::Set(model.#field_name)
            });
        }
    }

    Ok(quote! {
        impl Model {
            /// Save (update) this model instance to the database.
            ///
            /// This updates ALL fields, following Django's behavior.
            /// Fields marked with #[auto_now] are automatically updated to the current timestamp.
            pub async fn save<'a, C: ::sea_orm::ConnectionTrait>(
                self,
                db: &'a C,
            ) -> ::core::result::Result<Self, ::seaorm_django::error::DjangoOrmError> {
                let now = ::chrono::Utc::now().fixed_offset();
                let model = self;

                let mut active_model = ActiveModel {
                    #(#save_assignments,)*
                };

                // Update auto_now fields
                #(#auto_now_updates)*

                use ::sea_orm::ActiveModelTrait;
                ::core::result::Result::Ok(active_model.update(db).await?)
            }
        }
    })
}

/// Generate convenience methods and constants on Model for better UX
fn generate_model_convenience_methods(
    input: &DeriveInput,
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
        _ => {
            return Err(syn::Error::new_spanned(
                input,
                "Only structs are supported",
            ))
        }
    };

    // Generate column constants: Book::Title = Column::Title
    let column_constants: Vec<_> = fields
        .iter()
        .map(|field| {
            let field_name = field.ident.as_ref().unwrap();
            let constant_name = format_ident!("{}", to_pascal_case(&field_name.to_string()));
            quote! {
                pub const #constant_name: Column = Column::#constant_name;
            }
        })
        .collect();

    Ok(quote! {
        impl Entity {
            // Column constants for convenient access: Book::Title instead of book::Column::Title
            #(#column_constants)*
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
