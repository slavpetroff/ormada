//! Test DjangoModel derive with relations using the new macro
use seaorm_django::prelude::*;

pub mod author {
    use super::*;

    #[django_model(table = "authors")]
    pub struct Author {
        #[primary_key]
        pub id: i32,
        pub name: String,
    }

    impl AsyncLifecycleHooks for Model {}
}

pub mod book {
    use super::*;

    #[django_model(table = "books")]
    pub struct Book {
        #[primary_key]
        pub id: i32,
        pub title: String,
        #[foreign_key(super::author::Author)]
        pub author_id: i32,
    }

    impl AsyncLifecycleHooks for Model {}
}

#[test]
fn test_compiles() {
    // Just ensure the derive macro works with relations
    // The macro expansion check happens at compile time
}
