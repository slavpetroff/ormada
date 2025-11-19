//! Integration tests for query operations

use crate::common::author::{Column as AuthorColumn, Entity as Author};
use crate::common::*;
use seaorm_django::prelude::*;

#[tokio::test]
async fn test_all_returns_all_records() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    // Leak db to get 'static lifetime for testing
    let db: &'static _ = Box::leak(Box::new(db));

    let authors = Author::objects(db).all().await.unwrap();
    assert_eq!(authors.len(), 3);
}

#[tokio::test]
async fn test_all_on_empty_table() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let authors = Author::objects(db).all().await.unwrap();
    assert_eq!(authors.len(), 0);
}

#[tokio::test]
async fn test_first_returns_first_record() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let first = Author::objects(db).first().await.unwrap();
    assert_eq!(first.name, "Alice Johnson");
}

#[tokio::test]
async fn test_first_on_empty_table_returns_error() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let result = Author::objects(db).first().await;
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("not found") || err_msg.contains("No records found"));
}

#[tokio::test]
async fn test_last_returns_last_record() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let last = Author::objects(db).last().await.unwrap();
    assert_eq!(last.name, "Charlie Brown");
}

#[tokio::test]
async fn test_last_on_empty_table_returns_error() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let result = Author::objects(db).last().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_count_returns_correct_count() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let count = Author::objects(db).count().await.unwrap();
    assert_eq!(count, 3);
}

#[tokio::test]
async fn test_count_on_empty_table() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let count = Author::objects(db).count().await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_exists_returns_true_when_records_exist() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let exists = Author::objects(db).exists().await.unwrap();
    assert!(exists);
}

#[tokio::test]
async fn test_exists_returns_false_on_empty_table() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let exists = Author::objects(db).exists().await.unwrap();
    assert!(!exists);
}

#[tokio::test]
async fn test_filter_with_eq() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let authors = Author::objects(db)
        .filter(AuthorColumn::Name.eq("Bob Smith"))
        .all()
        .await
        .unwrap();

    assert_eq!(authors.len(), 1);
    assert_eq!(authors[0].name, "Bob Smith");
}

#[tokio::test]
async fn test_filter_with_contains() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let authors = Author::objects(db)
        .filter(AuthorColumn::Email.contains("example.com"))
        .all()
        .await
        .unwrap();

    assert_eq!(authors.len(), 3);
}

#[tokio::test]
async fn test_filter_with_gt() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let authors = Author::objects(db)
        .filter(AuthorColumn::Age.gt(30))
        .all()
        .await
        .unwrap();

    assert_eq!(authors.len(), 2); // Alice (35) and Bob (42)
}

#[tokio::test]
async fn test_filter_with_lt() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let authors = Author::objects(db)
        .filter(AuthorColumn::Age.lt(35))
        .all()
        .await
        .unwrap();

    assert_eq!(authors.len(), 1); // Charlie (28)
}

#[tokio::test]
async fn test_filter_with_range() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let authors = Author::objects(db)
        .filter(AuthorColumn::Age.gte(30))
        .filter(AuthorColumn::Age.lte(40))
        .all()
        .await
        .unwrap();

    assert_eq!(authors.len(), 1); // Alice (35)
}

#[tokio::test]
async fn test_exclude() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let authors = Author::objects(db)
        .exclude(AuthorColumn::Name.eq("Bob Smith"))
        .all()
        .await
        .unwrap();

    assert_eq!(authors.len(), 2);
}

#[tokio::test]
async fn test_order_by_asc() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let authors = Author::objects(db)
        .order_by_asc(AuthorColumn::Age)
        .all()
        .await
        .unwrap();

    assert_eq!(authors[0].age, 28); // Charlie
    assert_eq!(authors[1].age, 35); // Alice
    assert_eq!(authors[2].age, 42); // Bob
}

#[tokio::test]
async fn test_order_by_desc() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let authors = Author::objects(db)
        .order_by_desc(AuthorColumn::Age)
        .all()
        .await
        .unwrap();

    assert_eq!(authors[0].age, 42); // Bob
    assert_eq!(authors[1].age, 35); // Alice
    assert_eq!(authors[2].age, 28); // Charlie
}

#[tokio::test]
async fn test_limit() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let authors = Author::objects(db).limit(2).all().await.unwrap();

    assert_eq!(authors.len(), 2);
}

#[tokio::test]
async fn test_limit_greater_than_total() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let authors = Author::objects(db).limit(100).all().await.unwrap();

    assert_eq!(authors.len(), 3);
}

#[tokio::test]
async fn test_offset() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    // Note: SQLite requires LIMIT when using OFFSET
    let authors = Author::objects(db)
        .limit(10) // Add large limit to allow offset to work
        .offset(1)
        .all()
        .await
        .unwrap();

    assert_eq!(authors.len(), 2);
}

#[tokio::test]
async fn test_offset_greater_than_total() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    // Note: SQLite requires LIMIT when using OFFSET
    let authors = Author::objects(db)
        .limit(10) // Add large limit
        .offset(100)
        .all()
        .await
        .unwrap();

    assert_eq!(authors.len(), 0);
}

#[tokio::test]
async fn test_chained_operations() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let authors = Author::objects(db)
        .filter(AuthorColumn::Age.gt(25))
        .exclude(AuthorColumn::Name.eq("Bob Smith"))
        .order_by_asc(AuthorColumn::Age)
        .limit(10)
        .all()
        .await
        .unwrap();

    assert_eq!(authors.len(), 2); // Charlie and Alice
    assert_eq!(authors[0].name, "Charlie Brown");
    assert_eq!(authors[1].name, "Alice Johnson");
}
