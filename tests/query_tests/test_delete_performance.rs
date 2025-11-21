//! Tests for optimized delete performance

use crate::common::*;
use sea_orm::ColumnTrait;
use seaorm_django::query::QueryExt;

#[tokio::test]
async fn test_delete_uses_bulk_in_clause() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    // Delete all authors - should use bulk DELETE with IN clause
    let count = Author::objects(&db)
        .filter(Author::Age.gte(20))
        .delete()
        .await
        .unwrap();

    assert!(count > 0);

    // Verify deletion
    let remaining = Author::objects(&db).all().await.unwrap();
    assert_eq!(remaining.len(), 0);
}

#[tokio::test]
async fn test_delete_empty_result() {
    let db = setup_test_db().await;

    // Delete with filter that matches nothing
    let count = Author::objects(&db)
        .filter(Author::Age.gt(999))
        .delete()
        .await
        .unwrap();

    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_delete_single_record() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let target_id = authors[0].id;

    // Delete single record by ID
    let count = Author::objects(&db)
        .filter(Author::Id.eq(target_id))
        .delete()
        .await
        .unwrap();

    assert_eq!(count, 1);

    // Verify only that one was deleted
    let remaining = Author::objects(&db).all().await.unwrap();
    assert_eq!(remaining.len(), 2);
}

#[tokio::test]
async fn test_update_with_select_for_update() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let initial_age = authors[0].age;

    // Update should use SELECT FOR UPDATE
    let count = Author::objects(&db)
        .filter(Author::Id.eq(authors[0].id))
        .update(|author| {
            author.age += 10;
        })
        .await
        .unwrap();

    assert_eq!(count, 1);

    // Verify update - author 0 has age 35, so should be 45 after +10
    let updated = Author::objects(&db)
        .filter(Author::Id.eq(authors[0].id))
        .first()
        .await
        .unwrap();
    
    assert_eq!(updated.age, 45);
}

#[tokio::test]
async fn test_update_multiple_records() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    // Update all authors
    let count = Author::objects(&db)
        .update(|author| {
            author.age = 50;
        })
        .await
        .unwrap();

    assert_eq!(count, 3);

    // Verify all updated
    let all = Author::objects(&db).all().await.unwrap();
    for author in all {
        assert_eq!(author.age, 50);
    }
}
