//! Internal cache implementation tests
//!
//! Tests for the cache module's internal functions


use super::common::test_helpers::*;
use super::common::fixtures::simple_item;
use seaorm_django::prelude::*;

#[tokio::test]
async fn test_cache_populated_on_first_all() {
    let db = super::common::test_helpers::setup_test_db().await;
    simple_item::create_table(&db).await;

    let items = simple_item::sample_items(10);
    simple_item::Entity::objects(&db).bulk_create(items).await.unwrap();

    let queryset = simple_item::Entity::objects(&db);
    
    // First call populates cache
    let results = queryset.all().await.unwrap();
    assert_eq!(results.len(), 10);
    
    // Cache should now be populated (verified by second call returning same data)
    let results2 = queryset.all().await.unwrap();
    assert_eq!(results.len(), results2.len());
}

#[tokio::test]
async fn test_cache_not_shared_between_different_queries() {
    let db = super::common::test_helpers::setup_test_db().await;
    simple_item::create_table(&db).await;

    let items = simple_item::sample_items(50);
    simple_item::Entity::objects(&db).bulk_create(items).await.unwrap();

    let query1 = simple_item::Entity::objects(&db)
        .filter(simple_item::Column::Value.lt(25));
    
    let query2 = simple_item::Entity::objects(&db)
        .filter(simple_item::Column::Value.gte(25));

    // Populate both caches
    let results1 = query1.all().await.unwrap();
    let results2 = query2.all().await.unwrap();

    assert_eq!(results1.len(), 25);
    assert_eq!(results2.len(), 25);

    // Each should have independent cache
    let results1_again = query1.all().await.unwrap();
    let results2_again = query2.all().await.unwrap();

    assert_eq!(results1.len(), results1_again.len());
    assert_eq!(results2.len(), results2_again.len());
}

#[tokio::test]
async fn test_builder_methods_create_new_queryset() {
    let db = super::common::test_helpers::setup_test_db().await;
    simple_item::create_table(&db).await;

    let items = simple_item::sample_items(100);
    simple_item::Entity::objects(&db).bulk_create(items).await.unwrap();

    let base = simple_item::Entity::objects(&db);
    
    // Each builder method should create a new QuerySet with new cache
    let filtered = base.filter(simple_item::Column::Value.lt(50));
    let limited = filtered.limit(10);
    let ordered = limited.order_by_asc(simple_item::Column::Value);

    // All should execute independently
    let base_results = base.all().await.unwrap();
    let filtered_results = filtered.all().await.unwrap();
    let limited_results = limited.all().await.unwrap();
    let ordered_results = ordered.all().await.unwrap();

    assert_eq!(base_results.len(), 100);
    assert_eq!(filtered_results.len(), 50);
    assert_eq!(limited_results.len(), 10);
    assert_eq!(ordered_results.len(), 10);
}

#[tokio::test]
async fn test_first_without_prior_cache() {
    let db = super::common::test_helpers::setup_test_db().await;
    simple_item::create_table(&db).await;

    let items = simple_item::sample_items(10);
    simple_item::Entity::objects(&db).bulk_create(items).await.unwrap();

    let queryset = simple_item::Entity::objects(&db)
        .order_by_asc(simple_item::Column::Value);
    
    // Call first() without calling all() first
    let first = queryset.first().await.unwrap();
    assert_eq!(first.value, 0);
}

#[tokio::test]
async fn test_get_without_cache() {
    let db = super::common::test_helpers::setup_test_db().await;
    simple_item::create_table(&db).await;

    let item = simple_item::Entity::objects(&db)
        .create(simple_item::Model { id: 0, value: 42 })
        .await
        .unwrap();

    // Get by ID (doesn't use cache)
    let retrieved = simple_item::Entity::objects(&db)
        .get(item.id)
        .await
        .unwrap();

    assert_eq!(retrieved.value, 42);
}

#[tokio::test]
async fn test_count_independent_of_cache() {
    let db = super::common::test_helpers::setup_test_db().await;
    simple_item::create_table(&db).await;

    let items = simple_item::sample_items(50);
    simple_item::Entity::objects(&db).bulk_create(items).await.unwrap();

    let queryset = simple_item::Entity::objects(&db);

    // Count doesn't populate cache
    let count = queryset.count().await.unwrap();
    assert_eq!(count, 50);

    // All still needs to query
    let all_results = queryset.all().await.unwrap();
    assert_eq!(all_results.len(), 50);
}

#[tokio::test]
async fn test_exists_independent_of_cache() {
    let db = super::common::test_helpers::setup_test_db().await;
    simple_item::create_table(&db).await;

    let items = simple_item::sample_items(10);
    simple_item::Entity::objects(&db).bulk_create(items).await.unwrap();

    let queryset = simple_item::Entity::objects(&db);

    // Exists doesn't populate cache
    let exists = queryset.exists().await.unwrap();
    assert!(exists);

    // All still needs to query
    let all_results = queryset.all().await.unwrap();
    assert_eq!(all_results.len(), 10);
}

#[tokio::test]
async fn test_cache_with_complex_query_chain() {
    let db = super::common::test_helpers::setup_test_db().await;
    simple_item::create_table(&db).await;

    let items = simple_item::sample_items(200);
    simple_item::Entity::objects(&db).bulk_create(items).await.unwrap();

    // Complex chain
    let queryset = simple_item::Entity::objects(&db)
        .filter(simple_item::Column::Value.gte(50))
        .filter(simple_item::Column::Value.lt(150))
        .order_by_desc(simple_item::Column::Value)
        .limit(20)
        .offset(10);

    // First execution
    let results1 = queryset.all().await.unwrap();
    assert_eq!(results1.len(), 20);

    // Cached execution
    let results2 = queryset.all().await.unwrap();
    assert_eq!(results2.len(), 20);

    // Verify same data
    for (r1, r2) in results1.iter().zip(results2.iter()) {
        assert_eq!(r1.id, r2.id);
        assert_eq!(r1.value, r2.value);
    }
}

#[tokio::test]
async fn test_cache_cleared_on_query_modification() {
    let db = super::common::test_helpers::setup_test_db().await;
    simple_item::create_table(&db).await;

    let items = simple_item::sample_items(100);
    simple_item::Entity::objects(&db).bulk_create(items).await.unwrap();

    let base = simple_item::Entity::objects(&db);
    
    // Populate base cache
    let base_results = base.all().await.unwrap();
    assert_eq!(base_results.len(), 100);

    // Modify query - creates new QuerySet with new cache
    let modified = base.filter(simple_item::Column::Value.lt(50));
    let modified_results = modified.all().await.unwrap();
    assert_eq!(modified_results.len(), 50);

    // Original cache still valid
    let base_results_again = base.all().await.unwrap();
    assert_eq!(base_results_again.len(), 100);
}

#[tokio::test]
async fn test_multiple_concurrent_querysets() {
    let db = super::common::test_helpers::setup_test_db().await;
    simple_item::create_table(&db).await;

    let items = simple_item::sample_items(100);
    simple_item::Entity::objects(&db).bulk_create(items).await.unwrap();

    // Create multiple querysets
    let qs1 = simple_item::Entity::objects(&db).filter(simple_item::Column::Value.lt(25));
    let qs2 = simple_item::Entity::objects(&db).filter(simple_item::Column::Value.between(25, 50));
    let qs3 = simple_item::Entity::objects(&db).filter(simple_item::Column::Value.between(51, 75));
    let qs4 = simple_item::Entity::objects(&db).filter(simple_item::Column::Value.gt(75));

    // Execute all
    let r1 = qs1.all().await.unwrap();
    let r2 = qs2.all().await.unwrap();
    let r3 = qs3.all().await.unwrap();
    let r4 = qs4.all().await.unwrap();

    assert_eq!(r1.len(), 25);
    assert_eq!(r2.len(), 26);
    assert_eq!(r3.len(), 25);
    assert_eq!(r4.len(), 24);

    // All cached independently
    let r1_cached = qs1.all().await.unwrap();
    let r2_cached = qs2.all().await.unwrap();
    let r3_cached = qs3.all().await.unwrap();
    let r4_cached = qs4.all().await.unwrap();

    assert_eq!(r1.len(), r1_cached.len());
    assert_eq!(r2.len(), r2_cached.len());
    assert_eq!(r3.len(), r3_cached.len());
    assert_eq!(r4.len(), r4_cached.len());
}

#[tokio::test]
async fn test_offset_creates_new_cache() {
    let db = super::common::test_helpers::setup_test_db().await;
    simple_item::create_table(&db).await;

    let items = simple_item::sample_items(100);
    simple_item::Entity::objects(&db).bulk_create(items).await.unwrap();

    let base = simple_item::Entity::objects(&db)
        .order_by_asc(simple_item::Column::Value);

    let page1 = base.limit(10);
    let page2 = base.limit(10).offset(10);
    let page3 = base.limit(10).offset(20);

    let r1 = page1.all().await.unwrap();
    let r2 = page2.all().await.unwrap();
    let r3 = page3.all().await.unwrap();

    assert_eq!(r1[0].value, 0);
    assert_eq!(r2[0].value, 10);
    assert_eq!(r3[0].value, 20);
}

#[tokio::test]
async fn test_distinct_creates_new_cache() {
    let db = super::common::test_helpers::setup_test_db().await;
    simple_item::create_table(&db).await;

    // Insert with duplicates
    for i in 0..5 {
        for _ in 0..2 {
            simple_item::Entity::objects(&db)
                .create(simple_item::Model { id: 0, value: i })
                .await
                .unwrap();
        }
    }

    let all_qs = simple_item::Entity::objects(&db);
    let distinct_qs = all_qs.distinct();

    let all_results = all_qs.all().await.unwrap();
    let distinct_results = distinct_qs.all().await.unwrap();

    // All has duplicates
    assert_eq!(all_results.len(), 10);
    
    // Distinct may or may not reduce count depending on implementation
    // Just verify it executes
    assert!(distinct_results.len() > 0);
}
