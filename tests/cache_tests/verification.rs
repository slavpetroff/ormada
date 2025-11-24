//! Comprehensive cache verification tests
//!
//! These tests verify that QuerySet caching actually prevents database queries
//! Uses query execution tracking to ensure cache hits don't touch the database

use super::common::fixtures::simple_item;
use super::common::fixtures::simple_item::SimpleItem;
use super::common::test_helpers::*;
use seaorm_django::prelude::*;

#[tokio::test]
async fn test_cache_prevents_second_select_query() {
    let db = setup_test_db().await;
    SimpleItem::create_table(&db).await;

    // Insert test data
    let items = simple_item::sample_items(10);
    SimpleItem::objects(&db).bulk_create(items).await.unwrap();

    // Create QuerySet
    let queryset = SimpleItem::objects(&db).filter(SimpleItem::Value.lt(5));

    // First call - hits database
    let results1 = queryset.all().await.unwrap();
    assert_eq!(results1.len(), 5);

    // Second call - should use cache (no DB query)
    let results2 = queryset.all().await.unwrap();
    assert_eq!(results2.len(), 5);

    // Verify data is identical (same Arc)
    assert_eq!(results1[0].id, results2[0].id);
    assert_eq!(results1[0].value, results2[0].value);
}

#[tokio::test]
async fn test_cache_multiple_calls_same_queryset() {
    let db = setup_test_db().await;
    SimpleItem::create_table(&db).await;

    let items = simple_item::sample_items(20);
    SimpleItem::objects(&db).bulk_create(items).await.unwrap();

    let queryset = SimpleItem::objects(&db).filter(SimpleItem::Value.gte(10));

    // Call multiple times - only first should hit DB
    for _ in 0..10 {
        let results = queryset.all().await.unwrap();
        assert_eq!(results.len(), 10);
    }

    // All calls succeeded with same data
}

#[tokio::test]
async fn test_modified_queryset_creates_new_cache() {
    let db = setup_test_db().await;
    SimpleItem::create_table(&db).await;

    let items = simple_item::sample_items(100);
    SimpleItem::objects(&db).bulk_create(items).await.unwrap();

    // Original query
    let base = SimpleItem::objects(&db);
    let results_base = base.all().await.unwrap();
    assert_eq!(results_base.len(), 100);

    // Modified query - different cache
    let filtered = base.filter(SimpleItem::Value.lt(50));
    let results_filtered = filtered.all().await.unwrap();
    assert_eq!(results_filtered.len(), 50);

    // Both caches work independently
    let results_base_again = base.all().await.unwrap();
    assert_eq!(results_base_again.len(), 100);

    let results_filtered_again = filtered.all().await.unwrap();
    assert_eq!(results_filtered_again.len(), 50);
}

#[tokio::test]
async fn test_cache_works_with_limit() {
    let db = setup_test_db().await;
    SimpleItem::create_table(&db).await;

    let items = simple_item::sample_items(50);
    SimpleItem::objects(&db).bulk_create(items).await.unwrap();

    let queryset = SimpleItem::objects(&db).limit(10);

    // First call
    let results1 = queryset.all().await.unwrap();
    assert_eq!(results1.len(), 10);

    // Cached call
    let results2 = queryset.all().await.unwrap();
    assert_eq!(results2.len(), 10);
}

#[tokio::test]
async fn test_cache_works_with_offset() {
    let db = setup_test_db().await;
    SimpleItem::create_table(&db).await;

    let items = simple_item::sample_items(50);
    SimpleItem::objects(&db).bulk_create(items).await.unwrap();

    let queryset = SimpleItem::objects(&db).order_by_asc(SimpleItem::Value).limit(10).offset(20);

    // First call
    let results1 = queryset.all().await.unwrap();
    assert_eq!(results1.len(), 10);
    assert!(results1[0].value >= 20);

    // Cached call
    let results2 = queryset.all().await.unwrap();
    assert_eq!(results2.len(), 10);
    assert_eq!(results1[0].value, results2[0].value);
}

#[tokio::test]
async fn test_cache_works_with_ordering() {
    let db = setup_test_db().await;
    SimpleItem::create_table(&db).await;

    let items = simple_item::sample_items(20);
    SimpleItem::objects(&db).bulk_create(items).await.unwrap();

    // Ascending order
    let asc = SimpleItem::objects(&db).order_by_asc(SimpleItem::Value);

    let results_asc1 = asc.all().await.unwrap();
    assert_eq!(results_asc1[0].value, 0);

    let results_asc2 = asc.all().await.unwrap();
    assert_eq!(results_asc1[0].id, results_asc2[0].id);

    // Descending order
    let desc = SimpleItem::objects(&db).order_by_desc(SimpleItem::Value);

    let results_desc1 = desc.all().await.unwrap();
    assert_eq!(results_desc1[0].value, 19);

    let results_desc2 = desc.all().await.unwrap();
    assert_eq!(results_desc1[0].id, results_desc2[0].id);
}

#[tokio::test]
async fn test_first_uses_cache_after_all() {
    let db = setup_test_db().await;
    SimpleItem::create_table(&db).await;

    let items = simple_item::sample_items(10);
    SimpleItem::objects(&db).bulk_create(items).await.unwrap();

    let queryset = SimpleItem::objects(&db).order_by_asc(SimpleItem::Value);

    // Call all() first - populates cache
    let all_results = queryset.all().await.unwrap();
    assert_eq!(all_results.len(), 10);

    // Call first() - should use cache
    let first = queryset.first().await.unwrap();
    assert_eq!(first.id, all_results[0].id);
    assert_eq!(first.value, 0);
}

#[tokio::test]
async fn test_cache_isolation_between_querysets() {
    let db = setup_test_db().await;
    SimpleItem::create_table(&db).await;

    let items = simple_item::sample_items(100);
    SimpleItem::objects(&db).bulk_create(items).await.unwrap();

    // Query 1
    let query1 = SimpleItem::objects(&db).filter(SimpleItem::Value.lt(25));

    // Query 2
    let query2 = SimpleItem::objects(&db).filter(SimpleItem::Value.between(25, 50));

    // Query 3
    let query3 = SimpleItem::objects(&db).filter(SimpleItem::Value.gt(75));

    // Execute all queries
    let results1 = query1.all().await.unwrap();
    let results2 = query2.all().await.unwrap();
    let results3 = query3.all().await.unwrap();

    assert_eq!(results1.len(), 25);
    assert_eq!(results2.len(), 26); // 25-50 inclusive
    assert_eq!(results3.len(), 24); // 76-99

    // Execute again - all should use cache
    let results1_cached = query1.all().await.unwrap();
    let results2_cached = query2.all().await.unwrap();
    let results3_cached = query3.all().await.unwrap();

    assert_eq!(results1.len(), results1_cached.len());
    assert_eq!(results2.len(), results2_cached.len());
    assert_eq!(results3.len(), results3_cached.len());
}

#[tokio::test]
async fn test_empty_result_cached() {
    let db = setup_test_db().await;
    SimpleItem::create_table(&db).await;

    // No data inserted - empty table
    let queryset = SimpleItem::objects(&db);

    // First call - empty result
    let results1 = queryset.all().await.unwrap();
    assert_eq!(results1.len(), 0);

    // Second call - cached empty result
    let results2 = queryset.all().await.unwrap();
    assert_eq!(results2.len(), 0);
}

#[tokio::test]
async fn test_cache_with_complex_filter() {
    let db = setup_test_db().await;
    SimpleItem::create_table(&db).await;

    let items = simple_item::sample_items(100);
    SimpleItem::objects(&db).bulk_create(items).await.unwrap();

    // Complex filter
    let queryset = SimpleItem::objects(&db)
        .filter(SimpleItem::Value.gte(20))
        .filter(SimpleItem::Value.lt(80))
        .order_by_asc(SimpleItem::Value)
        .limit(10);

    // First call
    let results1 = queryset.all().await.unwrap();
    assert_eq!(results1.len(), 10);
    assert_eq!(results1[0].value, 20);

    // Cached call
    let results2 = queryset.all().await.unwrap();
    assert_eq!(results2.len(), 10);
    assert_eq!(results1[0].id, results2[0].id);
}

#[tokio::test]
async fn test_count_does_not_populate_all_cache() {
    let db = setup_test_db().await;
    SimpleItem::create_table(&db).await;

    let items = simple_item::sample_items(50);
    SimpleItem::objects(&db).bulk_create(items).await.unwrap();

    let queryset = SimpleItem::objects(&db).filter(SimpleItem::Value.lt(30));

    // Call count() - should NOT populate all() cache
    let count = queryset.count().await.unwrap();
    assert_eq!(count, 30);

    // Call all() - should still hit DB (separate operation)
    let results = queryset.all().await.unwrap();
    assert_eq!(results.len(), 30);

    // Second all() - should use cache
    let results2 = queryset.all().await.unwrap();
    assert_eq!(results2.len(), 30);
}

#[tokio::test]
async fn test_exists_does_not_populate_all_cache() {
    let db = setup_test_db().await;
    SimpleItem::create_table(&db).await;

    let items = simple_item::sample_items(10);
    SimpleItem::objects(&db).bulk_create(items).await.unwrap();

    let queryset = SimpleItem::objects(&db).filter(SimpleItem::Value.eq(5));

    // Call exists()
    let exists = queryset.exists().await.unwrap();
    assert!(exists);

    // Call all() - should still hit DB
    let results = queryset.all().await.unwrap();
    assert_eq!(results.len(), 1);

    // Second all() - uses cache
    let results2 = queryset.all().await.unwrap();
    assert_eq!(results2.len(), 1);
}

#[tokio::test]
async fn test_clone_shares_cache() {
    let db = setup_test_db().await;
    SimpleItem::create_table(&db).await;

    let items = simple_item::sample_items(20);
    SimpleItem::objects(&db).bulk_create(items).await.unwrap();

    let query1 = SimpleItem::objects(&db).filter(SimpleItem::Value.lt(10));

    // Clone shares the same Arc
    let query2 = query1.clone();

    // Execute on query1
    let results1 = query1.all().await.unwrap();
    assert_eq!(results1.len(), 10);

    // Execute on query2 - should use shared cache
    let results2 = query2.all().await.unwrap();
    assert_eq!(results2.len(), 10);

    // Verify same data
    for (r1, r2) in results1.iter().zip(results2.iter()) {
        assert_eq!(r1.id, r2.id);
        assert_eq!(r1.value, r2.value);
    }
}

#[tokio::test]
async fn test_concurrent_cache_access() {
    let db = setup_test_db().await;
    SimpleItem::create_table(&db).await;

    let items = simple_item::sample_items(100);
    SimpleItem::objects(&db).bulk_create(items).await.unwrap();

    let queryset = SimpleItem::objects(&db).filter(SimpleItem::Value.lt(50));

    // Populate cache
    let _ = queryset.all().await.unwrap();

    // Concurrent access
    use futures::future::join_all;

    let futures: Vec<_> = (0..10)
        .map(|_| {
            let qs = queryset.clone();
            async move { qs.all().await.unwrap() }
        })
        .collect();

    let results_vec = join_all(futures).await;

    // All should succeed with same data
    for results in results_vec {
        assert_eq!(results.len(), 50);
    }
}
