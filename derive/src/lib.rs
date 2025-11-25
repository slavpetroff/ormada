//! Proc macro for #[derive(DjangoModel)]
//!
//! This crate provides a derive macro that automatically generates
//! Model-based create/update operations with auto field handling.

// Proc macros are allowed to use patterns that would be problematic in regular code
// This is standard practice for code generation
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::disallowed_methods)]
#![allow(clippy::option_if_let_else)]
#![allow(clippy::use_self)]
#![allow(clippy::ref_option)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::redundant_clone)]
#![allow(clippy::single_char_pattern)]
#![allow(clippy::unnecessary_wraps)]
#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::explicit_iter_loop)]
#![allow(clippy::collection_is_never_read)]
#![allow(clippy::suspicious_doc_comments)]
#![allow(clippy::while_let_on_iterator)]
#![allow(clippy::manual_while_let_some)]
#![allow(clippy::unused_peekable)]
#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_must_use)]

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

mod atomic;
mod model;
mod projection;
mod relations;

/// Check if a field has a specific `sea_orm` attribute
fn has_sea_orm_attribute(field: &syn::Field, attr_name: &str) -> bool {
    for attr in &field.attrs {
        if attr.path().is_ident("sea_orm") {
            // Simple string-based check for attribute presence
            let meta_str = quote::quote!(#attr).to_string();
            if meta_str.contains(attr_name) {
                return true;
            }
        }
    }
    false
}

/// Check if a field has a specific django attribute
fn has_django_attribute(field: &syn::Field, attr_name: &str) -> bool {
    for attr in &field.attrs {
        if attr.path().is_ident("django") {
            // Simple string-based check for attribute presence
            let meta_str = quote::quote!(#attr).to_string();
            if meta_str.contains(attr_name) {
                return true;
            }
        }
    }
    false
}

/// Derive macro for Django-like Model-based operations
#[proc_macro_derive(DjangoModel, attributes(django))]
pub fn derive_django_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let struct_name = &input.ident;

    // Extract fields from the struct
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return syn::Error::new_spanned(
                    struct_name,
                    "DjangoModel can only be derived for structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(
                struct_name,
                "DjangoModel can only be derived for structs",
            )
            .to_compile_error()
            .into();
        }
    };

    // Categorize fields
    let mut primary_key = None;
    let mut auto_now_add_fields = Vec::new(); // Set on create only
    let mut auto_now_fields = Vec::new(); // Set on create AND update
    let mut all_fields = Vec::new();

    for field in fields {
        let field_name = field.ident.as_ref().unwrap();
        let field_ty = &field.ty;

        all_fields.push((field_name, field_ty));

        // Detect primary key from #[sea_orm(primary_key)]
        if has_sea_orm_attribute(field, "primary_key") {
            primary_key = Some(field_name);
            continue;
        }

        // Detect auto fields from #[django(...)] attributes
        if has_django_attribute(field, "auto_now_add") {
            auto_now_add_fields.push(field_name);
            continue;
        }

        if has_django_attribute(field, "auto_now") {
            auto_now_fields.push(field_name);
        }
    }

    // Primary key is required
    if primary_key.is_none() {
        return syn::Error::new_spanned(
            struct_name,
            "Model must have a field marked with #[sea_orm(primary_key)]",
        )
        .to_compile_error()
        .into();
    }
    let primary_key = primary_key.unwrap();

    // Note: Type-specific column traits can't be implemented on enum variants
    // For now, we'll keep the generic ColumnExt trait approach
    // This is still type-safe at compile time, just not at the method level

    // Generate ActiveModel field assignments for create
    let create_field_assignments: Vec<_> = all_fields
        .iter()
        .map(|(field_name, _)| {
            if Some(*field_name) == Some(primary_key) {
                // If ID is default/zero, let DB handle it (NotSet)
                // Otherwise use the provided ID
                // We need fully qualified Default call to avoid ambiguity with PartialEq
                let field_ty = all_fields.iter().find(|(n, _)| n == field_name).unwrap().1;
                quote! {
                    #field_name: if model.#field_name == <#field_ty as Default>::default() {
                        sea_orm::ActiveValue::NotSet
                    } else {
                        sea_orm::ActiveValue::Set(model.#field_name)
                    }
                }
            } else if auto_now_add_fields.contains(field_name)
                || auto_now_fields.contains(field_name)
            {
                quote! { #field_name: sea_orm::ActiveValue::Set(now) }
            } else {
                quote! { #field_name: sea_orm::ActiveValue::Set(model.#field_name) }
            }
        })
        .collect();

    // Generate ActiveModel field assignments for save (update all fields)
    let save_field_assignments: Vec<_> = all_fields
        .iter()
        .map(|(field_name, _)| {
            if Some(*field_name) == Some(primary_key) {
                // Primary key must be Set for update to work
                quote! { #field_name: sea_orm::ActiveValue::Set(self.#field_name) }
            } else if auto_now_fields.contains(field_name) {
                // auto_now fields will be set below with current timestamp
                quote! { #field_name: sea_orm::ActiveValue::Set(now) }
            } else {
                // All other fields: update with current value
                quote! { #field_name: sea_orm::ActiveValue::Set(self.#field_name) }
            }
        })
        .collect();

    // Generate ActiveModel field assignments for update (only auto_now fields)
    let update_auto_fields = auto_now_fields.iter().map(|field_name| {
        quote! {
            active_model.#field_name = sea_orm::ActiveValue::Set({
                let now: sea_orm::prelude::DateTimeWithTimeZone = chrono::Utc::now().into();
                now
            });
        }
    });

    // Parse relations
    let relation_infos = relations::parse_relations(&input);

    // ALWAYS generate ModelWithRelations (needed even for entities without relations)
    let model_with_relations = relations::generate_model_with_relations(fields, &relation_infos);
    let from_impl = relations::generate_from_impl(fields, &relation_infos);

    // ALWAYS generate WithRelationsTrait (needed for accessor methods to work)
    let field_refs: Vec<&syn::Field> = fields.iter().collect();
    let trait_impl = relations::generate_trait_impl(&relation_infos, &field_refs);

    // Generate HasRelation implementations (compile-time typed relations)
    let has_relation_impls = relations::generate_has_relation_impls(&relation_infos);

    let expanded = quote! {
        // ===== RELATION MODELS =====
        #model_with_relations
        #from_impl
        #trait_impl

        // ===== DJANGO ENTITY TRAIT =====
        impl ::seaorm_django::traits::DjangoEntity for Entity {
            fn to_active_model_for_create(model: Model) -> ::core::result::Result<ActiveModel, ::seaorm_django::error::DjangoOrmError> {
                let now = ::chrono::Utc::now().fixed_offset();
                ::core::result::Result::Ok(ActiveModel {
                    #(#create_field_assignments,)*
                })
            }

            async fn save_model<'a, C: ::sea_orm::ConnectionTrait>(
                db: &'a C,
                model: Model,
            ) -> Result<Model, seaorm_django::error::DjangoOrmError> {
                model.save(db).await
            }
        }

        // ===== UPDATE OPERATION =====
        impl Model {
            /// Save (update) this model (Django-style: updates ALL fields)
            ///
            /// All model fields are updated in the database.
            /// Fields marked with #[django(auto_now)] are automatically set to the current timestamp.
            ///
            /// This follows Django's behavior where .save() updates all fields,
            /// not just modified ones.
            pub async fn save<'a, C: ::sea_orm::ConnectionTrait>(
                self,
                db: &'a C,
            ) -> Result<Self, seaorm_django::error::DjangoOrmError> {
                use sea_orm::Set;
                let now = ::chrono::Utc::now().fixed_offset();

                // Create ActiveModel with ALL fields marked as Set (to be updated)
                let mut active_model = ActiveModel {
                    #(#save_field_assignments,)*
                };

                // Override auto_now fields with current timestamp
                #(#update_auto_fields)*

                use sea_orm::ActiveModelTrait;
                Ok(active_model.update(db).await?)
            }
        }

        // ===== RELATION LOADING =====
        #has_relation_impls
    };

    TokenStream::from(expanded)
}

/// Attribute macro for atomic transactions (Django's @transaction.atomic)
///
/// Wraps the function body in a transaction.
///
/// # Usage
///
/// ```rust,ignore
/// #[atomic(db)]
/// async fn create_user(db: &DatabaseConnection, name: String) -> Result<(), DjangoOrmError> {
///     // This code runs inside a transaction!
///     // 'db' is shadowed by the transaction handle
///     let user = User::objects(db).create(name).await?;
///     Ok(())
/// }
/// ```
#[proc_macro_attribute]
pub fn atomic(args: TokenStream, input: TokenStream) -> TokenStream {
    atomic::impl_atomic(args, input)
}

/// Attribute macro for defining Django-like models with clean syntax
///
/// This macro transforms a simple struct definition into a full `SeaORM` entity
/// with all the necessary derives and boilerplate.
///
/// # Model Attributes
///
/// - `table = "table_name"` - **(required)** Database table name
/// - `ordering = "field"` - Default ordering for queries
/// - `hooks = true` - Enable custom lifecycle hooks (see below)
///
/// # Usage
///
/// ```rust,ignore
/// use seaorm_django::prelude::*;
///
/// #[django_model(table = "books")]
/// struct Book {
///     #[primary_key]
///     id: i32,
///
///     #[max_length(200)]
///     #[index]
///     title: String,
///
///     #[foreign_key(Author, on_delete = Cascade)]
///     author_id: i32,
///
///     #[auto_now_add]
///     created_at: DateTimeWithTimeZone,
///
///     #[auto_now]
///     updated_at: DateTimeWithTimeZone,
/// }
/// ```
///
/// # Lifecycle Hooks
///
/// By default, an empty `LifecycleHooks` implementation is auto-generated.
/// To provide custom hooks, use `hooks = true`:
///
/// ```rust,ignore
/// #[django_model(table = "books", hooks = true)]
/// struct Book { /* fields */ }
///
/// #[async_trait]
/// impl LifecycleHooks for book::Model {
///     async fn before_save(&mut self) -> Result<(), DjangoOrmError> {
///         // Custom logic before save
///         Ok(())
///     }
/// }
/// ```
///
/// # Field Attributes
///
/// - `#[primary_key]` - Mark field as primary key
/// - `#[primary_key(auto_increment = false)]` - Control auto-increment
/// - `#[foreign_key(Model)]` - Define foreign key relationship (use Model type, not Entity)
/// - `#[foreign_key(Model, on_delete = Cascade)]` - FK with ON DELETE behavior
/// - `#[index]` / `#[index(name = "idx_name")]` - Create index
/// - `#[unique]` / `#[unique(name = "uniq_name")]` - Unique constraint
/// - `#[max_length(n)]` - String max length validation
/// - `#[min_length(n)]` - String min length validation
/// - `#[range(min = n, max = m)]` - Numeric range validation
/// - `#[auto_now]` - Auto-update timestamp on save
/// - `#[auto_now_add]` - Auto-set timestamp on creation
/// - `#[soft_delete]` - Mark field for soft delete (must be `Option<DateTimeWithTimeZone>`)
/// - `#[skip_serializing]` - Skip field when serializing
/// - `#[skip_deserializing]` - Skip field when deserializing
#[proc_macro_attribute]
pub fn django_model(attr: TokenStream, item: TokenStream) -> TokenStream {
    match model::impl_django_model(attr.into(), item.into()) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Attribute macro for defining type-safe projections with compile-time validation
///
/// Provides a type-safe alternative to JSON-based `values()` queries.
/// Validates that all non-computed fields exist on the model at compile time.
///
/// # Usage
///
/// ```rust,ignore
/// use seaorm_django::prelude::*;
///
/// // Simple projection (all fields must exist on Book)
/// #[django_projection(model = Book)]
/// struct BookSummary {
///     title: String,
///     price: f64,
/// }
///
/// // With computed fields (for aggregations)
/// #[django_projection(model = Book)]
/// struct AuthorBookStats {
///     author_id: i32,           // Validated
///     #[computed]
///     book_count: i64,          // Not validated (computed by DB)
///     #[computed]
///     avg_price: Option<f64>,   // Not validated (computed by DB)
/// }
///
/// // Query usage
/// let summaries: Vec<BookSummary> = Book::objects(db)
///     .filter(Book::Published.eq(true))
///     .project::<BookSummary>()
///     .await?;
/// ```
///
/// # Field Attributes
///
/// - `#[computed]` - Mark field as computed (e.g., aggregations). These fields
///   are not validated against the model and must be provided by the query
///   (e.g., via `.annotate()` for aggregations).
#[proc_macro_attribute]
pub fn django_projection(attr: TokenStream, item: TokenStream) -> TokenStream {
    match projection::generate_projection(attr.into(), item.into()) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}
