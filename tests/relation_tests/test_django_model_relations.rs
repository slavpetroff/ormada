//! Test DjangoModel derive with relations

use sea_orm::entity::prelude::*;
use seaorm_django_derive::DjangoModel;

pub mod author {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, DjangoModel)]
    #[sea_orm(table_name = "authors")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub name: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod book {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, DjangoModel)]
    #[sea_orm(table_name = "books")]
    #[django(relations(author = "super::author::Entity"))]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub title: String,
        pub author_id: i32,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[test]
fn test_compiles() {
    // Just ensure the derive macro works with relations
}
