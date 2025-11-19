//! Tests for ColumnExt trait methods to achieve coverage
//!
//! Tests various column comparison and filter methods

use seaorm_django::prelude::*;
use seaorm_django::query::ColumnExt;

use crate::common::*;

// ============================================================================
// String Methods
// ============================================================================

#[tokio::test]
async fn test_starts_with() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let results = author::Entity::objects(db)
        .filter(ColumnExt::starts_with(&author::Column::Name, "Alice"))
        .all()
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    assert!(results[0].name.starts_with("Alice"));
}

#[tokio::test]
async fn test_ends_with() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let results = author::Entity::objects(db)
        .filter(ColumnExt::ends_with(&author::Column::Name, "Johnson"))
        .all()
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    assert!(results[0].name.ends_with("Johnson"));
}

#[tokio::test]
async fn test_contains() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let results = author::Entity::objects(db)
        .filter(ColumnExt::contains(&author::Column::Name, "Bob"))
        .all()
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    assert!(results[0].name.contains("Bob"));
}

// ============================================================================
// Comparison Methods
// ============================================================================

#[tokio::test]
async fn test_eq_method() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let results = author::Entity::objects(db)
        .filter(ColumnExt::eq(&author::Column::Id, authors[0].id))
        .all()
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, authors[0].id);
}

#[tokio::test]
async fn test_ne_method() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let results = author::Entity::objects(db)
        .filter(ColumnExt::ne(&author::Column::Id, authors[0].id))
        .all()
        .await
        .unwrap();

    assert_eq!(results.len(), 2);
    assert!(!results.iter().any(|a| a.id == authors[0].id));
}

#[tokio::test]
async fn test_gt_method() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let results = author::Entity::objects(db)
        .filter(ColumnExt::gt(&author::Column::Id, 0))
        .all()
        .await
        .unwrap();

    assert_eq!(results.len(), 3);
}

#[tokio::test]
async fn test_gte_method() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let results = author::Entity::objects(db)
        .filter(ColumnExt::gte(&author::Column::Id, authors[1].id))
        .all()
        .await
        .unwrap();

    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_lt_method() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let results = author::Entity::objects(db)
        .filter(ColumnExt::lt(&author::Column::Id, authors[2].id))
        .all()
        .await
        .unwrap();

    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_lte_method() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let results = author::Entity::objects(db)
        .filter(ColumnExt::lte(&author::Column::Id, authors[1].id))
        .all()
        .await
        .unwrap();

    assert_eq!(results.len(), 2);
}

// ============================================================================
// In Values Test
// ============================================================================

#[tokio::test]
async fn test_in_values_method() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let ids = vec![authors[0].id, authors[2].id];
    let results = author::Entity::objects(db)
        .filter(ColumnExt::in_values(&author::Column::Id, ids))
        .all()
        .await
        .unwrap();

    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|a| a.id == authors[0].id));
    assert!(results.iter().any(|a| a.id == authors[2].id));
}

// ============================================================================
// Null Checks
// ============================================================================

#[tokio::test]
async fn test_is_null_method() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    // Create author with null email (if possible)
    let results = author::Entity::objects(db)
        .filter(ColumnExt::is_null(&author::Column::Email))
        .all()
        .await
        .unwrap();

    // Our test data doesn't have nulls, so should be 0
    assert_eq!(results.len(), 0);
}

#[tokio::test]
async fn test_is_not_null_method() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let results = author::Entity::objects(db)
        .filter(ColumnExt::is_not_null(&author::Column::Name))
        .all()
        .await
        .unwrap();

    assert_eq!(results.len(), 3);
}
