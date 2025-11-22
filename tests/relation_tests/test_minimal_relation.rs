//! Minimal test to isolate the E0223 error
use seaorm_django::prelude::*;

// First: Test entity without relations (should work)
pub mod simple {
    use super::*;

    #[django_model(table = "simple")]
    pub struct Simple {
        #[primary_key]
        pub id: i32,
        pub name: String,
    }
    impl AsyncLifecycleHooks for Model {}
}

// Second: Add a related entity (should also work since it has no relations)
pub mod parent {
    use super::*;

    #[django_model(table = "parent")]
    pub struct Parent {
        #[primary_key]
        pub id: i32,
        pub name: String,
    }
    impl AsyncLifecycleHooks for Model {}
}

// Third: Add entity WITH relation to parent
pub mod child {
    use super::*;

    #[django_model(table = "child")]
    pub struct Child {
        #[primary_key]
        pub id: i32,
        pub name: String,
        #[foreign_key(super::parent::Parent)]
        pub parent_id: i32,
    }
    impl AsyncLifecycleHooks for Model {}
}

#[test]
fn test_simple_compiles() {
    // Just ensure entities compile
}
