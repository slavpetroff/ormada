//! Minimal test to isolate the E0223 error

use sea_orm::entity::prelude::*;
use seaorm_django_derive::DjangoModel;

// First: Test entity without relations (should work)
pub mod simple {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, DjangoModel)]
    #[sea_orm(table_name = "simple")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub name: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

// Second: Add a related entity (should also work since it has no relations)
pub mod parent {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, DjangoModel)]
    #[sea_orm(table_name = "parent")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub name: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

// Third: Add entity WITH relation to parent
pub mod child {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, DjangoModel)]
    #[sea_orm(table_name = "child")]
    #[django(relations(parent = "super::parent::Entity"))]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub name: String,
        pub parent_id: i32,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[test]
fn test_simple_compiles() {
    // Just ensure entities compile (child commented out temporarily)
}
