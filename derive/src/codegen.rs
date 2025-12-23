//! Code generation utilities for the ormada_model macro
//!
//! This module contains helpers that generate `SeaORM`-compatible code
//! using paths through `::ormada::__internal::` instead of `::sea_orm::`.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

/// Convert snake_case to PascalCase for enum variants
pub fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect()
}

/// Information about a field for code generation
#[derive(Clone)]
pub struct FieldInfo {
    pub name: Ident,
    pub ty: syn::Type,
    pub is_primary_key: bool,
    pub is_auto_increment: bool,
    pub is_nullable: bool,
    pub column_type: String,
}

/// Generate the Entity struct and EntityTrait implementation
pub fn generate_entity(table_name: &str, fields: &[FieldInfo]) -> TokenStream {
    let table_name_ident = format_ident!("{}", table_name);

    quote! {
        /// Database entity marker type
        #[derive(Copy, Clone, Default, Debug)]
        pub struct Entity;

        impl ::ormada::__internal::EntityName for Entity {
            fn schema_name(&self) -> ::core::option::Option<&str> {
                ::core::option::Option::None
            }

            fn table_name(&self) -> &str {
                #table_name
            }
        }

        impl ::ormada::__internal::IdenStatic for Entity {
            fn as_str(&self) -> &str {
                #table_name
            }
        }

        impl ::ormada::__internal::Iden for Entity {
            fn unquoted(&self, s: &mut dyn ::std::fmt::Write) {
                ::std::write!(s, "{}", #table_name).unwrap();
            }
        }

        impl ::ormada::__internal::EntityTrait for Entity {
            type Model = Model;
            type Column = Column;
            type PrimaryKey = PrimaryKey;
            type Relation = Relation;
        }
    }
}

/// Generate the Column enum and ColumnTrait implementation
pub fn generate_column_enum(fields: &[FieldInfo]) -> TokenStream {
    let variants: Vec<_> = fields
        .iter()
        .map(|f| {
            let variant = format_ident!("{}", to_pascal_case(&f.name.to_string()));
            variant
        })
        .collect();

    let variant_strs: Vec<_> = fields.iter().map(|f| f.name.to_string()).collect();

    let column_defs: Vec<_> = fields
        .iter()
        .map(|f| {
            let col_type = get_column_type_tokens(&f.ty, f.is_nullable);
            col_type
        })
        .collect();

    quote! {
        /// Column enum for this entity
        #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
        pub enum Column {
            #(#variants,)*
        }

        impl ::ormada::__internal::IdenStatic for Column {
            fn as_str(&self) -> &str {
                match self {
                    #(Column::#variants => #variant_strs,)*
                }
            }
        }

        impl ::ormada::__internal::Iden for Column {
            fn unquoted(&self, s: &mut dyn ::std::fmt::Write) {
                ::std::write!(s, "{}", ::ormada::__internal::IdenStatic::as_str(self)).unwrap();
            }
        }

        impl ::ormada::__internal::IntoIden for Column {
            fn into_iden(self) -> ::ormada::__internal::sea_orm::sea_query::DynIden {
                ::ormada::__internal::sea_orm::sea_query::SeaRc::new(self)
            }
        }

        impl ::ormada::__internal::ColumnTrait for Column {
            type EntityName = Entity;

            fn def(&self) -> ::ormada::__internal::ColumnDef {
                match self {
                    #(Column::#variants => #column_defs,)*
                }
            }
        }

        impl ::core::iter::IntoIterator for Column {
            type Item = Column;
            type IntoIter = ::std::vec::IntoIter<Column>;

            fn into_iter(self) -> Self::IntoIter {
                vec![#(Column::#variants,)*].into_iter()
            }
        }

        impl Column {
            /// Iterate over all columns
            pub fn iter() -> impl ::core::iter::Iterator<Item = Column> {
                [#(Column::#variants,)*].into_iter()
            }
        }
    }
}

/// Get the ColumnDef tokens for a Rust type
fn get_column_type_tokens(ty: &syn::Type, is_nullable: bool) -> TokenStream {
    let type_str = quote!(#ty).to_string().replace(" ", "");

    let base_def = if type_str.contains("i32") || type_str.contains("i64") {
        quote! { ::ormada::__internal::ColumnType::Integer.def() }
    } else if type_str.contains("String") {
        quote! { ::ormada::__internal::ColumnType::String(::core::option::Option::None).def() }
    } else if type_str.contains("bool") {
        quote! { ::ormada::__internal::ColumnType::Boolean.def() }
    } else if type_str.contains("f32") || type_str.contains("f64") {
        quote! { ::ormada::__internal::ColumnType::Double.def() }
    } else if type_str.contains("DateTimeWithTimeZone") || type_str.contains("DateTime") {
        quote! { ::ormada::__internal::ColumnType::TimestampWithTimeZone.def() }
    } else if type_str.contains("Uuid") {
        quote! { ::ormada::__internal::ColumnType::Uuid.def() }
    } else {
        // Default to string for unknown types
        quote! { ::ormada::__internal::ColumnType::String(::core::option::Option::None).def() }
    };

    if is_nullable || type_str.starts_with("Option<") {
        quote! { #base_def.nullable() }
    } else {
        base_def
    }
}

/// Generate the PrimaryKey enum and PrimaryKeyTrait implementation
pub fn generate_primary_key(pk_fields: &[FieldInfo]) -> TokenStream {
    if pk_fields.is_empty() {
        return quote! {
            #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
            pub enum PrimaryKey {}

            impl ::ormada::__internal::PrimaryKeyTrait for PrimaryKey {
                type ValueType = i32;

                fn auto_increment() -> bool { true }
            }

            impl PrimaryKey {
                pub fn iter() -> impl ::core::iter::Iterator<Item = PrimaryKey> {
                    ::core::iter::empty()
                }
            }
        };
    }

    let variants: Vec<_> = pk_fields
        .iter()
        .map(|f| format_ident!("{}", to_pascal_case(&f.name.to_string())))
        .collect();

    let column_variants: Vec<_> = pk_fields
        .iter()
        .map(|f| format_ident!("{}", to_pascal_case(&f.name.to_string())))
        .collect();

    // Determine value type from first PK field
    let first_pk = &pk_fields[0];
    let value_type = &first_pk.ty;
    let auto_inc = first_pk.is_auto_increment;

    quote! {
        #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
        pub enum PrimaryKey {
            #(#variants,)*
        }

        impl ::ormada::__internal::PrimaryKeyTrait for PrimaryKey {
            type ValueType = #value_type;

            fn auto_increment() -> bool { #auto_inc }
        }

        impl ::ormada::__internal::PrimaryKeyToColumn for PrimaryKey {
            type Column = Column;

            fn into_column(self) -> Self::Column {
                match self {
                    #(PrimaryKey::#variants => Column::#column_variants,)*
                }
            }

            fn from_column(col: Self::Column) -> ::core::option::Option<Self> {
                match col {
                    #(Column::#column_variants => ::core::option::Option::Some(PrimaryKey::#variants),)*
                    _ => ::core::option::Option::None,
                }
            }
        }

        impl PrimaryKey {
            pub fn iter() -> impl ::core::iter::Iterator<Item = PrimaryKey> {
                [#(PrimaryKey::#variants,)*].into_iter()
            }
        }
    }
}

/// Generate the ActiveModel struct and ActiveModelTrait implementation
pub fn generate_active_model(fields: &[FieldInfo]) -> TokenStream {
    let field_names: Vec<_> = fields.iter().map(|f| &f.name).collect();
    let field_types: Vec<_> = fields.iter().map(|f| &f.ty).collect();

    let pascal_names: Vec<_> = fields
        .iter()
        .map(|f| format_ident!("{}", to_pascal_case(&f.name.to_string())))
        .collect();

    quote! {
        /// ActiveModel for database operations
        #[derive(Clone, Debug, Default)]
        pub struct ActiveModel {
            #(pub #field_names: ::ormada::__internal::ActiveValue<#field_types>,)*
        }

        impl ::ormada::__internal::ActiveModelTrait for ActiveModel {
            type Entity = Entity;

            fn take(&mut self, c: <Self::Entity as ::ormada::__internal::EntityTrait>::Column) -> ::ormada::__internal::ActiveValue<::ormada::__internal::Value> {
                match c {
                    #(Column::#pascal_names => {
                        let mut value = ::ormada::__internal::ActiveValue::NotSet;
                        ::core::mem::swap(&mut value, &mut self.#field_names);
                        value.into_value().unwrap()
                    },)*
                }
            }

            fn get(&self, c: <Self::Entity as ::ormada::__internal::EntityTrait>::Column) -> ::ormada::__internal::ActiveValue<::ormada::__internal::Value> {
                match c {
                    #(Column::#pascal_names => self.#field_names.clone().into_value().unwrap(),)*
                }
            }

            fn set(&mut self, c: <Self::Entity as ::ormada::__internal::EntityTrait>::Column, v: ::ormada::__internal::Value) {
                match c {
                    #(Column::#pascal_names => self.#field_names = ::ormada::__internal::ActiveValue::Set(v.unwrap()),)*
                }
            }

            fn not_set(&mut self, c: <Self::Entity as ::ormada::__internal::EntityTrait>::Column) {
                match c {
                    #(Column::#pascal_names => self.#field_names = ::ormada::__internal::ActiveValue::NotSet,)*
                }
            }

            fn is_not_set(&self, c: <Self::Entity as ::ormada::__internal::EntityTrait>::Column) -> bool {
                match c {
                    #(Column::#pascal_names => self.#field_names.is_not_set(),)*
                }
            }

            fn default() -> Self {
                Self {
                    #(#field_names: ::ormada::__internal::ActiveValue::NotSet,)*
                }
            }

            fn reset(&mut self, c: <Self::Entity as ::ormada::__internal::EntityTrait>::Column) {
                match c {
                    #(Column::#pascal_names => self.#field_names = ::ormada::__internal::ActiveValue::NotSet,)*
                }
            }
        }

        impl ::ormada::__internal::ActiveModelBehavior for ActiveModel {}

        impl ::core::convert::From<Model> for ActiveModel {
            fn from(model: Model) -> Self {
                Self {
                    #(#field_names: ::ormada::__internal::Set(model.#field_names),)*
                }
            }
        }

        impl ::ormada::__internal::IntoActiveModel<ActiveModel> for Model {
            fn into_active_model(self) -> ActiveModel {
                ActiveModel::from(self)
            }
        }
    }
}
