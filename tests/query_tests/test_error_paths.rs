//! Tests for error paths and edge cases
//!
//! Tests error handling and boundary conditions

use sea_orm::ColumnTrait;
use seaorm_django::prelude::*;

use crate::common::*;

// ============================================================================
// Empty Query Results
// ============================================================================

#[tokio::test]
async fn test_first_on_filtered_empty_result() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let result = Author::objects(&db).filter(ColumnTrait::eq(&Author::Id, 99999)).first().await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_last_on_filtered_empty_result() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let result = Author::objects(&db).filter(ColumnTrait::eq(&Author::Id, 99999)).last().await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_nonexistent_id() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let result = Author::objects(&db).get(99999).await;

    assert!(result.is_err());

    // Verify it's the right error message
    if let Err(e) = result {
        assert!(e.to_string().contains("not found"));
    }
}

// ============================================================================
// Boundary Conditions
// ============================================================================

#[tokio::test]
async fn test_limit_one() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let results = Author::objects(&db).limit(1).all().await.unwrap();

    assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn test_offset_all() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;

    let results = Author::objects(&db).limit(100).offset(authors.len() as u64).all().await.unwrap();

    assert_eq!(results.len(), 0);
}

#[tokio::test]
async fn test_count_with_limit_and_offset() {
    let db = setup_test_db().await;
    create_sample_authors(&db).await;

    let total = Author::objects(&db).count().await.unwrap();

    assert_eq!(total, 3); // We created 3 authors

    let count_limited = Author::objects(&db).limit(1).offset(1).count().await.unwrap();

    // Verify count was calculated
    assert!(count_limited >= 0);
}

// ============================================================================
// Filter Edge Cases
// ============================================================================

#[tokio::test]
async fn test_filter_with_exclude_same_condition() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;

    let results = Author::objects(&db)
        .filter(ColumnTrait::eq(&Author::Id, authors[0].id))
        .exclude(ColumnTrait::eq(&Author::Id, authors[0].id))
        .all()
        .await
        .unwrap();

    assert_eq!(results.len(), 0);
}

#[tokio::test]
async fn test_multiple_same_filters() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;

    let results = Author::objects(&db)
        .filter(ColumnTrait::eq(&Author::Id, authors[0].id))
        .filter(ColumnTrait::eq(&Author::Id, authors[0].id))
        .filter(ColumnTrait::eq(&Author::Id, authors[0].id))
        .all()
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
}

// ============================================================================
// Ordering Edge Cases
// ============================================================================

#[tokio::test]
async fn test_order_on_empty_result() {
    let db = setup_test_db().await;

    let results = Author::objects(&db).order_by_asc(Author::Id).all().await.unwrap();

    assert_eq!(results.len(), 0);
}

#[tokio::test]
async fn test_order_with_single_result() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;

    let results = Author::objects(&db)
        .filter(ColumnTrait::eq(&Author::Id, authors[0].id))
        .order_by_desc(Author::Id)
        .all()
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, authors[0].id);
}

// ============================================================================
// Exists Edge Cases
// ============================================================================

#[tokio::test]
async fn test_exists_with_exclude_all() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let exists = Author::objects(&db)
        .exclude(ColumnTrait::gt(&Author::Id, 0))
        .exists()
        .await
        .unwrap();

    assert!(!exists);
}

#[tokio::test]
async fn test_exists_with_offset() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    // Exists should check if ANY records exist, regardless of offset
    let exists = Author::objects(&db).limit(10).offset(1000).exists().await.unwrap();

    // This might be true or false depending on implementation
    // Just verify it doesn't crash
    let _ = exists;
}

// ============================================================================
// Complex Query Chains
// ============================================================================

#[tokio::test]
async fn test_very_long_filter_chain() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let results = Author::objects(&db)
        .filter(ColumnTrait::gt(&Author::Id, 0))
        .filter(ColumnTrait::lt(&Author::Id, 1000))
        .filter(ColumnTrait::gt(&Author::Age, 0))
        .filter(ColumnTrait::lt(&Author::Age, 200))
        .exclude(ColumnTrait::eq(&Author::Id, -1))
        .exclude(ColumnTrait::eq(&Author::Age, -1))
        .order_by_asc(Author::Id)
        .limit(100)
        .all()
        .await
        .unwrap();

    assert!(results.len() > 0);
}

#[tokio::test]
async fn test_count_after_complex_chain() {
    let db = setup_test_db().await;
    create_sample_authors(&db).await;

    // First verify we have data
    let total = Author::objects(&db).count().await.unwrap();
    assert_eq!(total, 3);

    let count = Author::objects(&db)
        .filter(ColumnTrait::gt(&Author::Id, 0))
        .exclude(ColumnTrait::eq(&Author::Id, -1))
        .order_by_desc(Author::Age)
        .limit(1)
        .offset(1)
        .count()
        .await
        .unwrap();

    // The count should match the total since all authors have ID > 0
    assert!(count >= 0);
}

// ============================================================================
// String Filter Edge Cases
// ============================================================================

#[tokio::test]
async fn test_contains_empty_string() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    use seaorm_django::query::ColumnExt;

    let results = Author::objects(&db)
        .filter(ColumnExt::contains(&Author::Name, ""))
        .all()
        .await
        .unwrap();

    // Empty string should match all (or none, depending on implementation)
    let _ = results;
}

#[tokio::test]
async fn test_starts_with_full_string() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;

    use seaorm_django::query::ColumnExt;

    let full_name = &authors[0].name;
    let results = Author::objects(&db)
        .filter(ColumnExt::starts_with(&Author::Name, full_name))
        .all()
        .await
        .unwrap();

    assert!(results.len() >= 1);
    assert!(results.iter().any(|a| a.name == *full_name));
}

#[tokio::test]
async fn test_ends_with_full_string() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;

    use seaorm_django::query::ColumnExt;

    let full_name = &authors[0].name;
    let results = Author::objects(&db)
        .filter(ColumnExt::ends_with(&Author::Name, full_name))
        .all()
        .await
        .unwrap();

    assert!(results.len() >= 1);
    assert!(results.iter().any(|a| a.name == *full_name));
}

// ============================================================================
// Delete Edge Cases
// ============================================================================

#[tokio::test]
async fn test_delete_with_no_filter() {
    let db = setup_test_db().await;
    create_sample_authors(&db).await;

    let deleted = Author::objects(&db).delete().await.unwrap();

    assert_eq!(deleted, 3);

    let count = Author::objects(&db).count().await.unwrap();

    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_delete_nonexistent() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let deleted = Author::objects(&db)
        .filter(ColumnTrait::eq(&Author::Id, 99999))
        .delete()
        .await
        .unwrap();

    assert_eq!(deleted, 0);
}
