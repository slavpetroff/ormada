//! Tests for QuerySet caching behavior
//!
//! Verifies Ormada-like automatic caching with concurrency safety

use super::common::test_helpers::*;
use ormada::prelude::*;
use std::sync::Arc;
use tokio::sync::Barrier;

// Define model in the test module
mod cache_test_item {
    use ormada::prelude::*;

    #[ormada_model(table = "cache_test_items")]
    pub struct CacheTestItem {
        #[primary_key]
        pub id: i32,

        #[index]
        pub value: i32,

        pub data: String,
    }

    impl AsyncLifecycleHooks for CacheTestItem {}
}

#[tokio::test]
async fn test_query_caching_basic() {
    let db = setup_test_db().await;
    use cache_test_item::CacheTestItem;

    CacheTestItem::create_table(&db).await.unwrap();

    // Seed data
    let items: Vec<_> = (0..100)
        .map(|i| CacheTestItem {
            value: i,
            data: format!("Item {}", i),
            ..Default::default()
        })
        .collect();

    CacheTestItem::objects(&db).bulk_create(items).await.expect("Failed to seed");

    // Create QuerySet
    let queryset = CacheTestItem::objects(&db).filter(CacheTestItem::Value.lt(10));

    // First call - should hit DB
    let results1 = queryset.all().await.expect("First query failed");
    assert_eq!(results1.len(), 10);

    // Second call - should use cache
    let results2 = queryset.all().await.expect("Second query failed");
    assert_eq!(results2.len(), 10);

    // Results should be equal
    assert_eq!(results1[0].id, results2[0].id);
    assert_eq!(results1[0].value, results2[0].value);
}

#[tokio::test]
async fn test_separate_caches_for_different_queries() {
    use cache_test_item::CacheTestItem;
    let db = setup_test_db().await;
    CacheTestItem::create_table(&db).await.unwrap();

    // Seed data
    let items: Vec<_> = (0..100)
        .map(|i| CacheTestItem {
            value: i,
            data: format!("Item {}", i),
            ..Default::default()
        })
        .collect();

    CacheTestItem::objects(&db).bulk_create(items).await.expect("Failed to seed");

    // Create base QuerySet
    let base = CacheTestItem::objects(&db);

    // Different queries should have separate caches
    let query1 = base.filter(CacheTestItem::Value.lt(10));
    let query2 = base.filter(CacheTestItem::Value.gte(10));

    let results1 = query1.all().await.expect("Query 1 failed");
    let results2 = query2.all().await.expect("Query 2 failed");

    assert_eq!(results1.len(), 10);
    assert_eq!(results2.len(), 90);

    // Cached queries should return same results
    let results1_cached = query1.all().await.expect("Query 1 cached failed");
    let results2_cached = query2.all().await.expect("Query 2 cached failed");

    assert_eq!(results1.len(), results1_cached.len());
    assert_eq!(results2.len(), results2_cached.len());
}

#[tokio::test]
async fn test_cache_with_first_method() {
    let db = setup_test_db().await;
    use cache_test_item::CacheTestItem;
    CacheTestItem::create_table(&db).await.unwrap();

    // Seed data
    let items: Vec<_> = (0..10)
        .map(|i| CacheTestItem {
            value: i,
            data: format!("Item {}", i),
            ..Default::default()
        })
        .collect();

    CacheTestItem::objects(&db).bulk_create(items).await.expect("Failed to seed");

    let queryset = CacheTestItem::objects(&db).order_by_asc(CacheTestItem::Value);

    // First call should populate cache
    let _all = queryset.all().await.expect("all() failed");

    // first() should use the cache
    let first = queryset.first().await.expect("first() failed");
    assert_eq!(first.value, 0);
}

#[tokio::test]
async fn test_concurrent_cache_access() {
    let db = setup_test_db().await;
    use cache_test_item::CacheTestItem;
    CacheTestItem::create_table(&db).await.unwrap();

    // Seed data
    let items: Vec<_> = (0..100)
        .map(|i| CacheTestItem {
            value: i,
            data: format!("Item {}", i),
            ..Default::default()
        })
        .collect();

    CacheTestItem::objects(&db).bulk_create(items).await.expect("Failed to seed");

    let queryset = CacheTestItem::objects(&db).filter(CacheTestItem::Value.lt(50));

    // Populate cache
    let _ = queryset.all().await.expect("Initial query failed");

    // Test concurrent access by spawning tasks synchronously
    // (Since QuerySet borrows db, we can't move it into async tasks easily)
    let barrier = Arc::new(Barrier::new(5));

    // Use join_all instead of spawning to avoid lifetime issues
    use futures::future::join_all;

    let futures: Vec<_> = (0..5)
        .map(|_| {
            let qs = queryset.clone();
            let barrier = barrier.clone();
            async move {
                // Wait for all tasks to be ready
                barrier.wait().await;

                // All tasks access cache simultaneously
                qs.all().await.expect("Concurrent query failed")
            }
        })
        .collect();

    let results_vec = join_all(futures).await;

    // All tasks should succeed
    for results in results_vec {
        assert_eq!(results.len(), 50);
    }
}

#[tokio::test]
async fn test_modified_query_creates_new_cache() {
    let db = setup_test_db().await;
    use cache_test_item::CacheTestItem;
    CacheTestItem::create_table(&db).await.unwrap();

    // Seed data
    let items: Vec<_> = (0..100)
        .map(|i| CacheTestItem {
            value: i,
            data: format!("Item {}", i),
            ..Default::default()
        })
        .collect();

    CacheTestItem::objects(&db).bulk_create(items).await.expect("Failed to seed");

    let base = CacheTestItem::objects(&db).filter(CacheTestItem::Value.lt(50));

    // Populate base cache
    let base_results = base.all().await.expect("Base query failed");
    assert_eq!(base_results.len(), 50);

    // Modified query should have separate cache
    let limited = base.limit(10);
    let limited_results = limited.all().await.expect("Limited query failed");
    assert_eq!(limited_results.len(), 10);

    // Base cache should still work
    let base_results_cached = base.all().await.expect("Base cached failed");
    assert_eq!(base_results_cached.len(), 50);

    // Limited cache should work
    let limited_results_cached = limited.all().await.expect("Limited cached failed");
    assert_eq!(limited_results_cached.len(), 10);
}

#[tokio::test]
async fn test_cache_with_count() {
    let db = setup_test_db().await;
    use cache_test_item::CacheTestItem;
    CacheTestItem::create_table(&db).await.unwrap();

    // Seed data
    let items: Vec<_> = (0..100)
        .map(|i| CacheTestItem {
            value: i,
            data: format!("Item {}", i),
            ..Default::default()
        })
        .collect();

    CacheTestItem::objects(&db).bulk_create(items).await.expect("Failed to seed");

    let queryset = CacheTestItem::objects(&db).filter(CacheTestItem::Value.lt(25));

    // count() doesn't populate the all() cache
    let count = queryset.count().await.expect("count() failed");
    assert_eq!(count, 25);

    // all() should still work
    let results = queryset.all().await.expect("all() failed");
    assert_eq!(results.len(), 25);
}

#[tokio::test]
async fn test_cache_with_exists() {
    let db = setup_test_db().await;
    use cache_test_item::CacheTestItem;
    CacheTestItem::create_table(&db).await.unwrap();

    // Seed one item
    CacheTestItem::objects(&db)
        .create(CacheTestItem {
            value: 42,
            data: "Test".to_string(),
            ..Default::default()
        })
        .await
        .expect("Failed to create");

    let queryset = CacheTestItem::objects(&db).filter(CacheTestItem::Value.eq(42));

    // exists() should work
    let exists = queryset.exists().await.expect("exists() failed");
    assert!(exists);

    // all() should still work
    let results = queryset.all().await.expect("all() failed");
    assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn test_queryset_clone_shares_cache() {
    let db = setup_test_db().await;
    use cache_test_item::CacheTestItem;
    CacheTestItem::create_table(&db).await.unwrap();

    // Seed data
    let items: Vec<_> = (0..10)
        .map(|i| CacheTestItem {
            value: i,
            data: format!("Item {}", i),
            ..Default::default()
        })
        .collect();

    CacheTestItem::objects(&db).bulk_create(items).await.expect("Failed to seed");

    let queryset1 = CacheTestItem::objects(&db).filter(CacheTestItem::Value.lt(5));

    // Clone the QuerySet
    let queryset2 = queryset1.clone();

    // Populate cache with first QuerySet
    let _ = queryset1.all().await.expect("Query 1 failed");

    // Clone should share the cache
    let results2 = queryset2.all().await.expect("Query 2 failed");
    assert_eq!(results2.len(), 5);
}
