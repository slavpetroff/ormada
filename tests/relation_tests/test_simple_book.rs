//! Test book without relations first
use seaorm_django::prelude::*;

#[django_model(table = "books")]
pub struct Book {
    #[primary_key]
    pub id: i32,
    pub title: String,
    pub author_id: i32,
}

impl AsyncLifecycleHooks for Model {}

#[test]
fn test_compiles() {
    // Just ensure the derive macro works
}
