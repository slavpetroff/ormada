//! Proc macro for #[derive(DjangoModel)]
//!
//! This crate provides a derive macro that automatically generates
//! Model-based create/update operations with auto field handling.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

mod atomic;
mod relations;

/// Check if a field has a specific sea_orm attribute
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
    let mut regular_fields = Vec::new();
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
            continue;
        }

        // Regular writable field
        regular_fields.push((field_name, field_ty));
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

    // Generate relation-specific code only if relations exist
    let relation_code = if !relation_infos.is_empty() {
        let loader_registrations = relations::generate_loader_registrations(&relation_infos);

        quote! {
            #has_relation_impls
            #loader_registrations
        }
    } else {
        // Even without relations, implement EnsureLoadersRegistered as a no-op
        quote! {
            impl ::seaorm_django::relations::EnsureLoadersRegistered for Entity {
                fn ensure_loaders_registered() {
                    // No loaders to register
                }
            }
        }
    };

    let expanded = quote! {
        // ===== RELATION MODELS =====
        #model_with_relations
        #from_impl
        #trait_impl

        // ===== DJANGO ENTITY TRAIT =====
        impl ::seaorm_django::traits::DjangoEntity for Entity {
            fn to_active_model_for_create(model: Model) -> ActiveModel {
                let now: sea_orm::prelude::DateTimeWithTimeZone = chrono::Utc::now().into();
                ActiveModel {
                    #(#create_field_assignments,)*
                }
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
                let now: sea_orm::prelude::DateTimeWithTimeZone = chrono::Utc::now().into();

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
        #relation_code
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
