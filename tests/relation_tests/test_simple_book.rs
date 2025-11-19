//! Test book without relations first

use sea_orm::entity::prelude::*;
use seaorm_django_derive::DjangoModel;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, DjangoModel)]
#[sea_orm(table_name = "books")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub title: String,
    pub author_id: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[test]
fn test_compiles() {
    // Just ensure the derive macro works
}
