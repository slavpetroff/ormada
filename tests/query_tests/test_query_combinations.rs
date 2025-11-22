//! Query combination tests to improve coverage
//!
//! Tests complex query combinations and edge cases

use sea_orm::ColumnTrait;
use seaorm_django::prelude::*;

use crate::common::*;

// ============================================================================
// Multiple Filter Combinations
// ============================================================================

#[tokio::test]
async fn test_multiple_filters_chained() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let results = Author::objects(db)
        .filter(ColumnTrait::gt(&Author::Id, 0))
        .filter(ColumnTrait::lt(&Author::Id, 100))
        .filter(ColumnTrait::gt(&Author::Age, 0))
        .all()
        .await
        .unwrap();

    assert!(results.len() > 0);
    for author in &results {
        assert!(author.id > 0 && author.id < 100);
        assert!(author.age > 0);
    }
}

#[tokio::test]
async fn test_filter_exclude_combination() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let results = Author::objects(db)
        .filter(ColumnTrait::gt(&Author::Id, 0))
        .exclude(ColumnTrait::eq(&Author::Id, authors[0].id))
        .exclude(ColumnTrait::eq(&Author::Id, authors[1].id))
        .all()
        .await
        .unwrap();

    assert!(!results.iter().any(|a| a.id == authors[0].id || a.id == authors[1].id));
}

#[tokio::test]
async fn test_multiple_excludes() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let results = Author::objects(db)
        .exclude(ColumnTrait::eq(&Author::Id, authors[0].id))
        .exclude(ColumnTrait::eq(&Author::Id, authors[1].id))
        .exclude(ColumnTrait::eq(&Author::Id, authors[2].id))
        .all()
        .await
        .unwrap();

    assert_eq!(results.len(), 0);
}

// ============================================================================
// Ordering Edge Cases
// ============================================================================

#[tokio::test]
async fn test_order_by_desc() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let results = Author::objects(db).order_by_desc(Author::Id).all().await.unwrap();

    if results.len() > 1 {
        // Descending order
        assert!(results[0].id >= results[1].id);
    }
}

#[tokio::test]
async fn test_order_with_filter() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let results = Author::objects(db)
        .filter(ColumnTrait::gt(&Author::Id, 0))
        .order_by_asc(Author::Id)
        .all()
        .await
        .unwrap();

    for i in 1..results.len() {
        assert!(results[i].id >= results[i - 1].id);
    }
}

// ============================================================================
// Limit and Offset Combinations
// ============================================================================

#[tokio::test]
async fn test_limit_with_filter() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let results = Author::objects(db)
        .filter(ColumnTrait::gt(&Author::Id, 0))
        .limit(2)
        .all()
        .await
        .unwrap();

    assert!(results.len() <= 2);
}

#[tokio::test]
async fn test_limit_offset_combination() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let all = Author::objects(db).order_by_asc(Author::Id).all().await.unwrap();

    if all.len() >= 2 {
        let paginated = Author::objects(db)
            .order_by_asc(Author::Id)
            .limit(10)
            .offset(1)
            .all()
            .await
            .unwrap();

        assert_eq!(paginated.len(), all.len() - 1);
        if paginated.len() > 0 {
            assert_eq!(paginated[0].id, all[1].id);
        }
    }
}

#[tokio::test]
async fn test_offset_with_limit_required() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    // SQLite requires LIMIT with OFFSET
    let results = Author::objects(db).limit(100).offset(1).all().await.unwrap();

    assert_eq!(results.len(), 2); // 3 total - 1 offset
}

// ============================================================================
// Count with Filters
// ============================================================================

#[tokio::test]
async fn test_count_with_filter() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let count = Author::objects(db)
        .filter(ColumnTrait::eq(&Author::Id, authors[0].id))
        .count()
        .await
        .unwrap();

    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_count_with_exclude() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let total = Author::objects(db).count().await.unwrap();
    let count = Author::objects(db)
        .exclude(ColumnTrait::eq(&Author::Id, authors[0].id))
        .count()
        .await
        .unwrap();

    assert_eq!(count, total - 1);
}

#[tokio::test]
async fn test_count_ignores_limit() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let total = Author::objects(db).count().await.unwrap();
    let count_with_limit = Author::objects(db).limit(1).count().await.unwrap();

    // Count should ignore limit
    assert_eq!(count_with_limit, total);
}

// ============================================================================
// Exists with Filters
// ============================================================================

#[tokio::test]
async fn test_exists_with_filter() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let exists = Author::objects(db)
        .filter(ColumnTrait::eq(&Author::Id, authors[0].id))
        .exists()
        .await
        .unwrap();

    assert!(exists);
}

#[tokio::test]
async fn test_exists_with_no_match() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let exists = Author::objects(db)
        .filter(ColumnTrait::eq(&Author::Id, 99999))
        .exists()
        .await
        .unwrap();

    assert!(!exists);
}

#[tokio::test]
async fn test_exists_ignores_limit() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let exists = Author::objects(db).limit(0).exists().await.unwrap();

    // Should still check for existence
    assert!(exists);
}

// ============================================================================
// First and Last with Combinations
// ============================================================================

#[tokio::test]
async fn test_first_with_filter() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let first = Author::objects(db)
        .filter(ColumnTrait::eq(&Author::Id, authors[1].id))
        .first()
        .await
        .unwrap();

    assert_eq!(first.id, authors[1].id);
}

#[tokio::test]
async fn test_first_with_order() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let first_asc = Author::objects(db).order_by_asc(Author::Id).first().await.unwrap();

    let all = Author::objects(db).order_by_asc(Author::Id).all().await.unwrap();

    assert_eq!(first_asc.id, all[0].id);
}

#[tokio::test]
async fn test_last_with_filter() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let last = Author::objects(db)
        .filter(ColumnTrait::eq(&Author::Id, authors[1].id))
        .last()
        .await
        .unwrap();

    assert_eq!(last.id, authors[1].id);
}

#[tokio::test]
async fn test_last_with_order() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let last_desc = Author::objects(db).order_by_desc(Author::Id).last().await.unwrap();

    let all = Author::objects(db).order_by_desc(Author::Id).all().await.unwrap();

    assert_eq!(last_desc.id, all[all.len() - 1].id);
}

// ============================================================================
// Complex Query Chains
// ============================================================================

#[tokio::test]
async fn test_full_query_chain() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let results = Author::objects(db)
        .filter(ColumnTrait::gt(&Author::Id, 0))
        .filter(ColumnTrait::lt(&Author::Id, 100))
        .exclude(ColumnTrait::eq(&Author::Age, -1))
        .order_by_asc(Author::Id)
        .limit(10)
        .all()
        .await
        .unwrap();

    assert!(results.len() > 0);
    assert!(results.len() <= 10);
}

#[tokio::test]
async fn test_filter_all_then_exclude_all() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let results = Author::objects(db)
        .filter(ColumnTrait::gt(&Author::Id, 0))
        .exclude(ColumnTrait::gt(&Author::Id, 0))
        .all()
        .await
        .unwrap();

    assert_eq!(results.len(), 0);
}

// ============================================================================
// Edge Cases
// ============================================================================

#[tokio::test]
async fn test_limit_larger_than_dataset() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let results = Author::objects(db).limit(1000).all().await.unwrap();

    assert_eq!(results.len(), authors.len());
}

#[tokio::test]
async fn test_filter_same_condition_twice() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let results = Author::objects(db)
        .filter(ColumnTrait::eq(&Author::Id, authors[0].id))
        .filter(ColumnTrait::eq(&Author::Id, authors[0].id))
        .all()
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, authors[0].id);
}

#[tokio::test]
async fn test_order_by_asc() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let results = Author::objects(db).order_by_asc(Author::Id).all().await.unwrap();

    if results.len() > 1 {
        // Ascending order
        assert!(results[0].id <= results[1].id);
    }
}
