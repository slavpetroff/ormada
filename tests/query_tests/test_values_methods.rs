//! Tests for values() and values_list() methods
//!
//! Tests Django-like column selection functionality

use sea_orm::ColumnTrait;
use seaorm_django::prelude::*;

use crate::common::*;

// ============================================================================
// values() Tests
// ============================================================================

#[tokio::test]
async fn test_values_basic() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let values = Author::objects(&db).values(vec![Author::Name, Author::Age]).await.unwrap();

    assert_eq!(values.len(), 3);

    // Verify each value is a JSON object with the selected fields
    for val in &values {
        assert!(val.is_object());
        assert!(val.get("name").is_some());
        assert!(val.get("age").is_some());
        // Should not include other fields
        assert!(val.get("id").is_none() || val.as_object().unwrap().len() == 2);
    }
}

#[tokio::test]
async fn test_values_single_column() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let values = Author::objects(&db).values(vec![Author::Name]).await.unwrap();

    assert_eq!(values.len(), 3);

    for val in &values {
        assert!(val.is_object());
        assert!(val.get("name").is_some());
    }
}

#[tokio::test]
async fn test_values_with_filter() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;

    let values = Author::objects(&db)
        .filter(ColumnTrait::eq(&Author::Id, authors[0].id))
        .values(vec![Author::Name, Author::Email])
        .await
        .unwrap();

    assert_eq!(values.len(), 1);
    assert_eq!(values[0].get("name").and_then(|v| v.as_str()), Some(authors[0].name.as_str()));
}

#[tokio::test]
async fn test_values_empty_result() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let values = Author::objects(&db)
        .filter(ColumnTrait::eq(&Author::Id, 9999))
        .values(vec![Author::Name])
        .await
        .unwrap();

    assert_eq!(values.len(), 0);
}

#[tokio::test]
async fn test_values_empty_columns() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let values = Author::objects(&db).values(vec![]).await.unwrap();

    assert_eq!(values.len(), 0);
}

#[tokio::test]
async fn test_values_with_ordering() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let values = Author::objects(&db)
        .order_by_desc(Author::Age)
        .values(vec![Author::Name, Author::Age])
        .await
        .unwrap();

    assert_eq!(values.len(), 3);

    // Verify ordering - first result should have highest age
    let first_age = values[0].get("age").and_then(|v| v.as_i64()).unwrap();
    let last_age = values[2].get("age").and_then(|v| v.as_i64()).unwrap();
    assert!(first_age >= last_age);
}

#[tokio::test]
async fn test_values_with_limit() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let values = Author::objects(&db).limit(2).values(vec![Author::Name]).await.unwrap();

    assert_eq!(values.len(), 2);
}

// ============================================================================
// values_list() Tests
// ============================================================================

#[tokio::test]
async fn test_values_list_basic() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let values = Author::objects(&db)
        .values_list(vec![Author::Name, Author::Age], false)
        .await
        .unwrap();

    assert_eq!(values.len(), 3);

    // Each value should be an array
    for val in &values {
        assert!(val.is_array());
        let arr = val.as_array().unwrap();
        assert_eq!(arr.len(), 2);
    }
}

#[tokio::test]
async fn test_values_list_flat() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let values = Author::objects(&db).values_list(vec![Author::Name], true).await.unwrap();

    assert_eq!(values.len(), 3);

    // Each value should be a string (not array)
    for val in &values {
        assert!(val.is_string() || val.is_number() || val.is_boolean());
        assert!(!val.is_array());
    }
}

#[tokio::test]
async fn test_values_list_flat_multiple_columns() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    // flat=true with multiple columns should return arrays (flat is ignored)
    let values = Author::objects(&db)
        .values_list(vec![Author::Name, Author::Age], true)
        .await
        .unwrap();

    assert_eq!(values.len(), 3);

    // Should return arrays since we have multiple columns
    for val in &values {
        assert!(val.is_array());
    }
}

#[tokio::test]
async fn test_values_list_with_filter() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let values = Author::objects(&db)
        .filter(ColumnTrait::gt(&Author::Age, 25))
        .values_list(vec![Author::Name], true)
        .await
        .unwrap();

    // Should have at least one author over 25
    assert!(values.len() >= 1);
    assert!(values.len() <= 3);
}

#[tokio::test]
async fn test_values_list_empty_result() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let values = Author::objects(&db)
        .filter(ColumnTrait::eq(&Author::Id, 9999))
        .values_list(vec![Author::Name], true)
        .await
        .unwrap();

    assert_eq!(values.len(), 0);
}

#[tokio::test]
async fn test_values_list_empty_columns() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let values = Author::objects(&db).values_list(vec![], false).await.unwrap();

    assert_eq!(values.len(), 0);
}

#[tokio::test]
async fn test_values_list_with_ordering() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let values = Author::objects(&db)
        .order_by_asc(Author::Name)
        .values_list(vec![Author::Name], true)
        .await
        .unwrap();

    assert_eq!(values.len(), 3);

    // Verify ordering
    for i in 0..values.len() - 1 {
        let curr = values[i].as_str().unwrap();
        let next = values[i + 1].as_str().unwrap();
        assert!(curr <= next);
    }
}

#[tokio::test]
async fn test_values_list_with_limit_offset() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let values = Author::objects(&db)
        .order_by_asc(Author::Id)
        .limit(2)
        .offset(1)
        .values_list(vec![Author::Name], true)
        .await
        .unwrap();

    assert_eq!(values.len(), 2);
}

// ============================================================================
// Combined Tests
// ============================================================================

#[tokio::test]
async fn test_values_vs_all() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    // Get all models
    let all_count = Author::objects(&db).all().await.unwrap().len();

    // Get values
    let values_count = Author::objects(&db).values(vec![Author::Name]).await.unwrap().len();

    assert_eq!(all_count, values_count);
}

#[tokio::test]
async fn test_values_list_single_vs_multiple() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    // Single column flat
    let single = Author::objects(&db).values_list(vec![Author::Name], true).await.unwrap();

    // Single column non-flat
    let single_array = Author::objects(&db).values_list(vec![Author::Name], false).await.unwrap();

    assert_eq!(single.len(), single_array.len());

    // Flat should be strings, non-flat should be arrays
    assert!(single[0].is_string());
    assert!(single_array[0].is_array());
}
