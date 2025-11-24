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
    impl AsyncLifecycleHooks for Model {}

    pub fn sample_items(count: usize) -> Vec<SimpleItem> {
        (0..count).map(|i| Model { id: 0, value: i as i32 }).collect()
    }
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
    impl AsyncLifecycleHooks for Model {}

    pub async fn create_table(db: &DatabaseRouter) {
        Model::create_table(db).await.unwrap();
    }

    pub fn sample_items(count: usize, base_name: &str) -> Vec<Model> {
        use crate::common::test_helpers::test_timestamp;
        let timestamp = test_timestamp();

        (0..count)
            .map(|i| Model {
                id: 0,
                value: i as i32,
                name: format!("{} {}", base_name, i),
                created_at: timestamp,
            })
            .collect()
    }
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
    impl AsyncLifecycleHooks for Model {}

    pub async fn create_table(db: &DatabaseRouter) {
        Model::create_table(db).await.unwrap();
    }

    pub fn sample_items_with_nulls(count: usize, category: i32) -> Vec<Model> {
        (0..count)
            .map(|i| Model {
                id: 0,
                int_value: if i % 2 == 0 { Some(i as i32) } else { None },
                dec_value: None,
                category,
            })
            .collect()
    }

    pub fn sample_items_no_nulls(count: usize, category: i32) -> Vec<Model> {
        (0..count)
            .map(|i| Model {
                id: 0,
                int_value: Some(i as i32 * 10),
                dec_value: Some(i as i64),
                category,
            })
            .collect()
    }
}

// Fixture tests are in tests/EXAMPLE_TEST.rs
