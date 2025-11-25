//! Test fixtures - reusable model definitions for testing
//!
//! This module contains commonly used test models that can be shared across tests.

use seaorm_django::prelude::*;

/// Simple test item model - used for basic CRUD tests
pub mod simple_item {
    use super::*;

    #[django_model(table = "simple_items")]
    pub struct SimpleItem {
        #[primary_key]
        pub id: i32,
        pub value: i32,
    }
    #[async_trait]
    impl LifecycleHooks for Model {}
}

/// Rich item model - for testing with multiple field types
pub mod rich_item {
    use super::*;

    #[django_model(table = "rich_items")]
    pub struct RichItem {
        #[primary_key]
        pub id: i32,
        pub value: i32,
        #[max_length(100)]
        pub name: String,
        pub created_at: DateTimeWithTimeZone,
    }
    #[async_trait]
    impl LifecycleHooks for Model {}
}

/// Aggregate test item - for testing aggregations with NULLs
pub mod agg_item {
    use super::*;

    #[django_model(table = "agg_items")]
    pub struct AggItem {
        #[primary_key]
        pub id: i32,
        pub int_value: Option<i32>,
        pub dec_value: Option<i64>,
        pub category: i32,
    }
    #[async_trait]
    impl LifecycleHooks for Model {}
}

// Fixture tests are in tests/EXAMPLE_TEST.rs
