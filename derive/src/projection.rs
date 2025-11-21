//! Implementation of the `#[django_projection]` attribute macro
//!
//! Provides type-safe field projections with compile-time validation.

use proc_macro2::TokenStream;
use quote::{quote, format_ident};
use syn::{
    parse::{Parse, ParseStream},
    Data, DeriveInput, Fields, Ident, Token, Attribute, Meta,
};

/// Configuration for the `#[django_projection]` attribute
#[derive(Clone)]
struct ProjectionConfig {
    model: syn::Path,
}

impl Parse for ProjectionConfig {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut model = None;

        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            let _: Token![=] = input.parse()?;

            let ident_str = ident.to_string();
            match ident_str.as_str() {
                "model" => {
                    let path: syn::Path = input.parse()?;
                    model = Some(path);
                }
                _ => {
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

        Ok(ProjectionConfig {
            model: model.ok_or_else(|| {
                syn::Error::new(input.span(), "Missing required 'model' attribute")
            })?,
        })
    }
}

/// Check if a field has the `#[computed]` attribute
fn is_computed_field(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if let Meta::Path(ref path) = attr.meta {
            path.is_ident("computed")
        } else {
            false
        }
    })
}

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

/// Generate the projection implementation
pub fn generate_projection(
    attr: TokenStream,
    input: TokenStream,
) -> syn::Result<TokenStream> {
    let config: ProjectionConfig = syn::parse2(attr)?;
    let input: DeriveInput = syn::parse2(input)?;

    let struct_name = &input.ident;
    let model_path = &config.model;

    // Extract fields from struct
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    input,
                    "django_projection only works with structs with named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                input,
                "django_projection only works with structs",
            ));
        }
    };

    // Separate regular fields from computed fields
    let mut validations = Vec::new();
    let mut column_selections = Vec::new();

    for field in fields {
        let field_name = field.ident.as_ref().unwrap();
        let is_computed = is_computed_field(&field.attrs);

        if !is_computed {
            // Generate compile-time validation for non-computed fields
            let column_name = format_ident!("{}", to_pascal_case(&field_name.to_string()));

            validations.push(quote! {
                let _ : #model_path = #model_path::#column_name;
            });

            column_selections.push(quote! {
                #model_path::#column_name
            });
        }
    }

    // Filter out #[computed] attributes from output
    let filtered_fields: Vec<_> = fields
        .iter()
        .map(|field| {
            let mut field = field.clone();
            field.attrs.retain(|attr| !attr.path().is_ident("computed"));
            field
        })
        .collect();

    // Generate the output
    Ok(quote! {
        #[derive(Clone, Debug, ::sea_orm::FromQueryResult)]
        pub struct #struct_name {
            #(#filtered_fields),*
        }

        impl #struct_name {
            /// Compile-time validation that all non-computed fields exist on the model
            const _VALIDATE_PROJECTION_FIELDS: () = {
                #(#validations)*
            };

            /// Get the columns to select for this projection (non-computed fields only)
            pub(crate) fn columns() -> ::std::vec::Vec<impl ::sea_orm::ColumnTrait> {
                ::std::vec![#(#column_selections),*]
            }
        }
    })
}

