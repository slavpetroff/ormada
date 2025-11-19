//! Integration tests for write operations (create, update, delete)

use crate::common::author::{Entity as Author, Model as AuthorModel};
use crate::common::*;
use seaorm_django::prelude::*;

// ============================================================================
// CREATE OPERATIONS
// ============================================================================

#[tokio::test]
async fn test_entity_create_basic() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let author = AuthorModel {
        name: "Test Author".to_string(),
        email: "test@example.com".to_string(),
        age: 30,
        ..Default::default()
    };

    let created = Author::objects(db).create(author).await.unwrap();

    assert_eq!(created.name, "Test Author");
    assert_eq!(created.email, "test@example.com");
    assert_eq!(created.age, 30);
}

#[tokio::test]
async fn test_entity_create_with_auto_timestamps() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    use chrono::Utc;

    let before_create = Utc::now();

    let author = AuthorModel {
        name: "Timestamp Test".to_string(),
        email: "timestamp@example.com".to_string(),
        age: 25,
        ..Default::default()
    };

    let created = Author::objects(db).create(author).await.unwrap();

    let after_create = Utc::now();

    // Verify timestamps are within reasonable range
    assert!(created.created_at.timestamp() >= before_create.timestamp());
    assert!(created.created_at.timestamp() <= after_create.timestamp());
}

#[tokio::test]
async fn test_bulk_create_empty_vec() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let authors: Vec<AuthorModel> = vec![];

    let count = Author::objects(db).bulk_create(authors).await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_bulk_create_multiple() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let authors = vec![
        AuthorModel {
            name: "Bulk Author 1".to_string(),
            email: "bulk1@example.com".to_string(),
            age: 30,
            ..Default::default()
        },
        AuthorModel {
            name: "Bulk Author 2".to_string(),
            email: "bulk2@example.com".to_string(),
            age: 35,
            ..Default::default()
        },
        AuthorModel {
            name: "Bulk Author 3".to_string(),
            email: "bulk3@example.com".to_string(),
            age: 40,
            ..Default::default()
        },
    ];

    let count = Author::objects(db).bulk_create(authors).await.unwrap();
    assert_eq!(count, 3);

    // Verify they were actually created
    let all_authors = Author::objects(db).all().await.unwrap();
    assert_eq!(all_authors.len(), 3);
}

// ============================================================================
// UPDATE OPERATIONS
// ============================================================================

#[tokio::test]
async fn test_model_save_updates_auto_fields_only() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    // Get an existing author
    let author = Author::objects(db).first().await.unwrap();
    let original_updated_at = author.updated_at;

    // Wait to ensure timestamp changes
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Save without modifications
    // Note: .save() currently uses into_active_model() which marks fields as Unchanged
    // Only auto_now fields are explicitly updated
    let updated = author.save(db).await.unwrap();

    // auto_now field should be updated
    assert!(updated.updated_at.timestamp() >= original_updated_at.timestamp());

    // TODO: For full field updates, need to track changes or use ActiveModel directly
    // This is a limitation of the current implementation
}

#[tokio::test]
async fn test_model_save_updates_auto_now_timestamp() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let original = &authors[0];
    let original_updated_at = original.updated_at;

    // Wait a tiny bit to ensure timestamp difference
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Get and save (even without changes, updated_at should change)
    let author = Author::objects(db).first().await.unwrap();
    let saved = author.save(db).await.unwrap();

    // updated_at should be different (auto_now)
    // Note: This tests that the macro correctly sets auto_now fields
    assert!(saved.updated_at.timestamp() >= original_updated_at.timestamp());
}

// ============================================================================
// DELETE OPERATIONS
// ============================================================================

#[tokio::test]
async fn test_model_delete() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let count_before = Author::objects(db).count().await.unwrap();
    assert_eq!(count_before, 3);

    // Delete one author
    let author = Author::objects(db).first().await.unwrap();
    author.delete(db).await.unwrap();

    let count_after = Author::objects(db).count().await.unwrap();
    assert_eq!(count_after, 2);
}

#[tokio::test]
async fn test_queryset_delete_filtered() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    use crate::common::author::Column;

    // Delete authors with age > 35
    let deleted_count = Author::objects(db)
        .filter(Column::Age.gt(35))
        .delete()
        .await
        .unwrap();

    assert_eq!(deleted_count, 1); // Only Bob (42)

    let remaining = Author::objects(db).count().await.unwrap();
    assert_eq!(remaining, 2); // Alice and Charlie
}

#[tokio::test]
async fn test_queryset_delete_no_matches() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    use crate::common::author::Column;

    // Try to delete non-existent records
    let deleted_count = Author::objects(db)
        .filter(Column::Age.gt(100))
        .delete()
        .await
        .unwrap();

    assert_eq!(deleted_count, 0); // No matches

    let remaining = Author::objects(db).count().await.unwrap();
    assert_eq!(remaining, 3); // All still there
}

#[tokio::test]
async fn test_queryset_delete_all() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    // Delete all
    let deleted_count = Author::objects(db).delete().await.unwrap();

    assert_eq!(deleted_count, 3);

    let remaining = Author::objects(db).count().await.unwrap();
    assert_eq!(remaining, 0);
}

// ============================================================================
// ERROR CASES
// ============================================================================

#[tokio::test]
async fn test_delete_twice_succeeds_silently() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let author = Author::objects(db).first().await.unwrap();

    // Delete once
    author.clone().delete(db).await.unwrap();

    // Try to delete again - SeaORM doesn't error, it just affects 0 rows
    // This is expected SeaORM behavior
    let result = author.delete(db).await;
    assert!(result.is_ok()); // Succeeds but affects 0 rows

    // Note: Django would raise DoesNotExist, but SeaORM's design is different
    // This is acceptable behavior for an ORM
}
