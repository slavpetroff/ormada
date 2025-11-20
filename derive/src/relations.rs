//! Relation loading code generation

use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

/// Information about a relation extracted from the entity
#[derive(Clone)]
pub struct RelationInfo {
    pub field_name: syn::Ident,
    pub related_entity: syn::Path,
    pub foreign_key_field: syn::Ident,
}

/// Parse relation information from django attributes
///
/// Looks for #[django(relations(field = "path::to::Entity", ...))]
pub fn parse_relations(input: &DeriveInput) -> Vec<RelationInfo> {
    let mut relations = Vec::new();

    // Look for #[django(relations(...))] on the struct
    for attr in &input.attrs {
        if !attr.path().is_ident("django") {
            continue;
        }

        // Parse the attribute content
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("relations") {
                // Parse the relations list
                let content;
                syn::parenthesized!(content in meta.input);

                while !content.is_empty() {
                    // Parse: field_name = "entity::path::Entity"
                    let field_name: syn::Ident = content.parse()?;
                    let _: syn::Token![=] = content.parse()?;
                    let entity_path: syn::LitStr = content.parse()?;

                    // Parse the entity path
                    let related_entity: syn::Path = syn::parse_str(&entity_path.value())
                        .map_err(|e| syn::Error::new(entity_path.span(), e))?;

                    // Infer FK field name from relation name (e.g., author -> author_id)
                    let fk_field_name = format!("{}_id", field_name);
                    let foreign_key_field = syn::Ident::new(&fk_field_name, field_name.span());

                    relations.push(RelationInfo {
                        field_name,
                        related_entity,
                        foreign_key_field,
                    });

                    // Check for comma or end
                    if content.peek(syn::Token![,]) {
                        let _: syn::Token![,] = content.parse()?;
                    }
                }
            }
            Ok(())
        });
    }

    relations
}

/// Generate ModelWithRelations struct
pub fn generate_model_with_relations(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    relations: &[RelationInfo],
) -> TokenStream {
    // ALWAYS generate ModelWithRelations, even without relations
    // This is needed so other entities can reference it in their accessors

    // Collect all original fields
    let original_fields: Vec<_> = fields
        .iter()
        .map(|field| {
            let name = &field.ident;
            let ty = &field.ty;
            let vis = &field.vis;
            quote! {
                #vis #name: #ty
            }
        })
        .collect();

    // Generate relation fields
    let relation_fields: Vec<_> = relations
        .iter()
        .map(|rel| {
            let field_name = &rel.field_name;
            let related_entity = &rel.related_entity;

            quote! {
                /// Prefetched relation field
                pub #field_name: ::core::option::Option<
                    <#related_entity as ::seaorm_django::traits::WithRelationsTrait>::ModelWithRelations
                >
            }
        })
        .collect();

    // Generate accessor methods for ergonomic access
    let accessor_methods: Vec<_> = relations
        .iter()
        .map(|rel| {
            let field_name = &rel.field_name;
            let related_entity = &rel.related_entity;
            let doc = format!("Get the {} relation if it was prefetched", field_name);

            quote! {
                #[doc = #doc]
                pub fn #field_name(&self) -> ::core::option::Option<
                    &<#related_entity as ::seaorm_django::traits::WithRelationsTrait>::ModelWithRelations
                > {
                    self.#field_name.as_ref()
                }
            }
        })
        .collect();

    quote! {
        /// Extended model with relation accessor methods
        ///
        /// This struct contains all original model fields as direct properties,
        /// plus methods to access prefetched relations.
        ///
        /// # Direct Field Access
        ///
        /// All model fields are directly accessible:
        ///
        /// ```rust,ignore
        /// let book = Book::objects(db)
        ///     .prefetch_related(relations![Author, Publisher])
        ///     .first()
        ///     .await?;
        ///
        /// println!("Title: {}", book.title);  // Direct field access
        ///
        /// // Fluent relation chaining:
        /// if let Some(publisher) = book.author()?.publisher() {
        ///     println!("Published by: {}", publisher.name);
        /// }
        /// ```
        #[derive(Clone, Debug)]
        pub struct ModelWithRelations {
            // All original model fields
            #(#original_fields,)*

            // Prefetched relation fields
            #(#relation_fields,)*
        }

        impl ModelWithRelations {
            // Auto-generated accessor methods for each relation
            #(#accessor_methods)*
        }
    }
}

/// Generate From<Model> for ModelWithRelations
///
/// This generates a simple From impl that creates an empty graph
/// Primarily used for testing and simple cases
pub fn generate_from_impl(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    relations: &[RelationInfo],
) -> TokenStream {
    let field_copies: Vec<_> = fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().unwrap();
            quote! { #name: model.#name }
        })
        .collect();

    // Initialize relation fields as None (no prefetch)
    let relation_nones: Vec<_> = relations
        .iter()
        .map(|rel| {
            let field_name = &rel.field_name;
            quote! { #field_name: ::core::option::Option::None }
        })
        .collect();

    quote! {
        impl ::core::convert::From<Model> for ModelWithRelations {
            fn from(model: Model) -> Self {
                Self {
                    #(#field_copies,)*
                    #(#relation_nones,)*
                }
            }
        }
    }
}

/// Generate WithRelationsTrait implementation
///
/// ALWAYS generates the trait impl, even for entities without relations
/// This is needed so accessor methods can call the trait method on related entities
pub fn generate_trait_impl(relations: &[RelationInfo], fields: &[&syn::Field]) -> TokenStream {
    // Determine Relations type based on number of relations
    let relations_type = if relations.is_empty() {
        quote! { () }
    } else if relations.len() == 1 {
        let related_entity = &relations[0].related_entity;
        quote! { ::rustc_hash::FxHashMap<i32, <#related_entity as ::sea_orm::EntityTrait>::Model> }
    } else {
        let hashmap_types: Vec<_> = relations
            .iter()
            .map(|rel| {
                let related_entity = &rel.related_entity;
                quote! {
                    ::rustc_hash::FxHashMap<i32, <#related_entity as ::sea_orm::EntityTrait>::Model>
                }
            })
            .collect();
        quote! { ( #(#hashmap_types),* ) }
    };

    // Generate relation field lookups for from_model_and_relations
    let relation_lookups: Vec<_> = relations
        .iter()
        .enumerate()
        .map(|(idx, rel)| {
            let field_name = &rel.field_name;
            let fk_field = &rel.foreign_key_field;
            let related_entity = &rel.related_entity;

            let access = if relations.len() == 1 {
                quote! { relations }
            } else {
                let index = syn::Index::from(idx);
                quote! { &relations.#index }
            };

            quote! {
                #field_name: #access
                    .get(&model.#fk_field)
                    .cloned()
                    .map(|m| <#related_entity as ::seaorm_django::traits::WithRelationsTrait>::from_model_and_relations(m, &()))
            }
        })
        .collect();

    // Generate field copies for from_model_and_relations
    let field_copies: Vec<_> = fields
        .iter()
        .map(|field| {
            let name = field.ident.as_ref().unwrap();
            quote! { #name: model.#name }
        })
        .collect();

    // Always generate, regardless of whether entity has relations
    quote! {
        impl ::seaorm_django::traits::WithRelationsTrait for Entity {
            type Model = Model;
            type ModelWithRelations = ModelWithRelations;
            type Relations = #relations_type;

            fn from_model_and_relations(
                model: Self::Model,
                relations: &Self::Relations,
            ) -> Self::ModelWithRelations {
                ModelWithRelations {
                    #(#field_copies,)*
                    #(#relation_lookups,)*
                }
            }
        }
    }
}

/// Generate HasRelation trait implementations
pub fn generate_has_relation_impls(relations: &[RelationInfo]) -> TokenStream {
    if relations.is_empty() {
        return quote! {};
    }

    let impls: Vec<_> = relations
        .iter()
        .map(|rel| {
            let related_entity = &rel.related_entity;
            let fk_field = &rel.foreign_key_field;

            quote! {
                impl ::seaorm_django::relations::HasRelation<#related_entity> for Entity {
                    type RelatedPK = <<#related_entity as ::sea_orm::EntityTrait>::PrimaryKey as ::sea_orm::PrimaryKeyTrait>::ValueType;
                    
                    fn get_foreign_key(model: &Self::Model) -> Self::RelatedPK {
                        model.#fk_field
                    }

                    async fn load_related<C: ::sea_orm::ConnectionTrait>(
                        models: &[Self::Model],
                        db: &C,
                    ) -> ::core::result::Result<
                        ::rustc_hash::FxHashMap<Self::RelatedPK, <#related_entity as ::sea_orm::EntityTrait>::Model>,
                        ::seaorm_django::error::DjangoOrmError
                    > {
                        use ::sea_orm::{EntityTrait, QueryFilter, ColumnTrait, PrimaryKeyToColumn, ModelTrait, Iterable};

                        let fk_values: ::std::vec::Vec<Self::RelatedPK> = models
                            .iter()
                            .map(|m| m.#fk_field)
                            .collect();

                        if fk_values.is_empty() {
                            return ::core::result::Result::Ok(::rustc_hash::FxHashMap::default());
                        }

                        // Get primary key column for filtering
                        let pk_cols: ::std::vec::Vec<_> = <#related_entity as ::sea_orm::EntityTrait>::PrimaryKey::iter()
                            .map(|pk| pk.into_column())
                            .collect();
                        let id_column = pk_cols[0];

                        let related_models = <#related_entity as ::sea_orm::EntityTrait>::find()
                            .filter(id_column.is_in(fk_values))
                            .all(db)
                            .await?;

                        // Build HashMap using primary key values (assuming id field)
                        ::core::result::Result::Ok(
                            related_models
                                .into_iter()
                                .map(|m| (m.id, m))
                                .collect()
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

// Loader registration removed - compile-time typed relations don't need runtime registration
