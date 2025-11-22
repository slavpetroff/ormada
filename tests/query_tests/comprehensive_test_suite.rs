//! Comprehensive test suite for complete coverage
//!
//! This test suite covers edge cases and ensures idiomatic Rust patterns

use sea_orm::ColumnTrait;
use seaorm_django::prelude::*;

use crate::common::*;

// ============================================================================
// QuerySet Boundary Tests
// ============================================================================

#[tokio::test]
async fn test_all_empty_table() {
    let db = setup_test_db().await;

    let results = Author::objects(&db).all().await.unwrap();
    assert_eq!(results.len(), 0);
}

#[tokio::test]
async fn test_filter_no_results() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let results = Author::objects(&db)
        .filter(ColumnTrait::eq(&Author::Id, 99999))
        .all()
        .await
        .unwrap();

    assert_eq!(results.len(), 0);
}

#[tokio::test]
async fn test_limit_zero() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let results = Author::objects(&db).limit(0).all().await.unwrap();

    assert_eq!(results.len(), 0);
}

#[tokio::test]
async fn test_offset_beyond_data() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    // SQLite requires LIMIT with OFFSET
    let results = Author::objects(&db).limit(100).offset(1000).all().await.unwrap();

    assert_eq!(results.len(), 0);
}

// ============================================================================
// First/Last Error Handling
// ============================================================================

#[tokio::test]
async fn test_first_empty_table_errors() {
    let db = setup_test_db().await;

    let result = Author::objects(&db).first().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_last_empty_table_errors() {
    let db = setup_test_db().await;

    let result = Author::objects(&db).last().await;
    assert!(result.is_err());
}

// ============================================================================
// Chaining and Combination Tests
// ============================================================================

#[tokio::test]
async fn test_filter_and_exclude() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;

    let results = Author::objects(&db)
        .filter(ColumnTrait::gt(&Author::Id, 0))
        .exclude(ColumnTrait::eq(&Author::Id, authors[0].id))
        .all()
        .await
        .unwrap();

    assert_eq!(results.len(), authors.len() - 1);
    assert!(!results.iter().any(|a| a.id == authors[0].id));
}

#[tokio::test]
async fn test_order_limit_offset_combination() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let results = Author::objects(&db)
        .order_by_asc(Author::Id)
        .limit(2)
        .offset(1)
        .all()
        .await
        .unwrap();

    assert!(results.len() <= 2);
}

// ============================================================================
// Count and Exists Edge Cases
// ============================================================================

#[tokio::test]
async fn test_count_empty_table() {
    let db = setup_test_db().await;

    let count = Author::objects(&db).count().await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_exists_empty_table() {
    let db = setup_test_db().await;

    let exists = Author::objects(&db).exists().await.unwrap();
    assert!(!exists);
}

#[tokio::test]
async fn test_exists_with_data() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let exists = Author::objects(&db).exists().await.unwrap();
    assert!(exists);
}

// ============================================================================
// Relation Edge Cases
// ============================================================================

#[tokio::test]
async fn test_prefetch_empty_result() {
    let db = setup_test_db().await;

    let books = Book::objects(&db).prefetch_related(relations![Author]).all().await.unwrap();

    assert_eq!(books.len(), 0);
}

#[tokio::test]
async fn test_prefetch_with_filter() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;

    let books = Book::objects(&db)
        .filter(ColumnTrait::eq(&Book::AuthorId, authors[0].id))
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    for book in &books {
        if book.author_id == authors[0].id {
            assert!(book.author.is_some());
        }
    }
}

#[tokio::test]
async fn test_prefetch_with_ordering() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;

    let books_asc = Book::objects(&db)
        .order_by_asc(Book::Id)
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    let books_desc = Book::objects(&db)
        .order_by_desc(Book::Id)
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    assert_eq!(books_asc.len(), books_desc.len());
}

// ============================================================================
// Delete Operations
// ============================================================================

#[tokio::test]
async fn test_delete_all_records() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    let count_before = Author::objects(&db).count().await.unwrap();
    assert!(count_before > 0);

    let deleted = Author::objects(&db).delete().await.unwrap();
    assert_eq!(deleted, count_before);

    let count_after = Author::objects(&db).count().await.unwrap();
    assert_eq!(count_after, 0);
}

#[tokio::test]
async fn test_delete_filtered_subset() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;

    let deleted = Author::objects(&db)
        .filter(ColumnTrait::eq(&Author::Id, authors[0].id))
        .delete()
        .await
        .unwrap();

    assert_eq!(deleted, 1);

    let remaining = Author::objects(&db).count().await.unwrap();
    assert_eq!(remaining, (authors.len() - 1) as u64);
}

// ============================================================================
// Model With Relations Tests
// ============================================================================

#[tokio::test]
async fn test_model_with_relations_has_all_fields() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;

    let books = Book::objects(&db).prefetch_related(relations![Author]).all().await.unwrap();

    for book in &books {
        // Test field access
        let _id = book.id;
        let _title = &book.title;
        let _author_id = book.author_id;

        // Test relation access
        if book.author_id > 0 {
            assert!(book.author.is_some());
        }
    }
}

#[tokio::test]
async fn test_model_with_relations_clone() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;

    let books = Book::objects(&db).prefetch_related(relations![Author]).all().await.unwrap();

    if let Some(book) = books.first() {
        let cloned = book.clone();
        assert_eq!(cloned.id, book.id);
        assert_eq!(cloned.title, book.title);
    }
}

// ============================================================================
// Save and Update Tests
// ============================================================================

#[tokio::test]
async fn test_save_updates_model() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;

    let mut author = authors[0].clone();
    let author_id = author.id;
    author.name = "Updated Name".to_string();
    author.save(&db).await.unwrap();

    let reloaded = Author::objects(&db)
        .filter(ColumnTrait::eq(&Author::Id, author_id))
        .first()
        .await
        .unwrap();

    assert_eq!(reloaded.name, "Updated Name");
}

#[tokio::test]
async fn test_save_multiple_times() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;

    let author_id = authors[0].id;

    for i in 1..=3 {
        let mut author = authors[0].clone();
        author.name = format!("Update {}", i);
        author.save(&db).await.unwrap();
    }

    let reloaded = Author::objects(&db)
        .filter(ColumnTrait::eq(&Author::Id, author_id))
        .first()
        .await
        .unwrap();

    assert_eq!(reloaded.name, "Update 3");
}

// ============================================================================
// Model Clone and From Tests
// ============================================================================

#[tokio::test]
async fn test_model_clone() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;

    let author = authors[0].clone();
    let cloned = author.clone();

    assert_eq!(cloned.id, author.id);
    assert_eq!(cloned.name, author.name);
}
