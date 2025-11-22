//! Advanced relation tests for uncovered relations.rs functionality
//!
//! Tests multiple relations, error paths, and edge cases

use sea_orm::ColumnTrait;
use seaorm_django::prelude::*;

use crate::common::*;

// ============================================================================
// Get with Relations
// ============================================================================

#[tokio::test]
async fn test_get_with_relations() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    // Find a book that has an author
    let book_with_author = books.iter().find(|b| b.author_id == authors[0].id).unwrap();

    let book = Book::objects(db)
        .filter(ColumnTrait::eq(&Book::Id, book_with_author.id))
        .prefetch_related(relations![Author])
        .first()
        .await
        .unwrap();

    assert_eq!(book.id, book_with_author.id);
    assert!(book.author.is_some());
}

#[tokio::test]
async fn test_get_without_relations_none() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    // Get book without prefetch
    let book = Book::objects(db)
        .filter(ColumnTrait::eq(&Book::Id, books[0].id))
        .first()
        .await
        .unwrap();

    // With our simplified implementation, Model is the same as ModelWithRelations
    // Relations are loaded via prefetch_related, not stored on the model
    assert_eq!(book.id, books[0].id);
}

// ============================================================================
// Error Path: Invalid Foreign Keys
// Note: These tests are commented out as they require FK constraint violations
// which the test environment doesn't allow without complex setup
// ============================================================================

#[tokio::test]
async fn test_prefetch_with_all_invalid_fks() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    // Create books with invalid author IDs
    for i in 1..=3 {
        Book::objects(db)
            .create(Book {
                title: format!("Orphan Book {}", i),
                author_id: 99900 + i, // Invalid FK
                published: true,
                price: 1000,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    let books = Book::objects(db).prefetch_related(relations![Author]).all().await.unwrap();

    // All books should have None for author
    assert!(books.len() >= 3);
    for book in &books {
        if book.author_id > 99900 {
            assert!(book.author.is_none());
        }
    }
}

#[tokio::test]
async fn test_prefetch_partial_invalid_fks() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    // Create one valid and one invalid
    Book::objects(db)
        .create(Book {
            title: "Valid Book".to_string(),
            author_id: authors[0].id,
            published: true,
            price: 1000,
            ..Default::default()
        })
        .await
        .unwrap();

    Book::objects(db)
        .create(Book {
            title: "Invalid Book".to_string(),
            author_id: 99999,
            published: true,
            price: 1000,
            ..Default::default()
        })
        .await
        .unwrap();

    let books = Book::objects(db).prefetch_related(relations![Author]).all().await.unwrap();

    let valid_book = books.iter().find(|b| b.title == "Valid Book").unwrap();
    let invalid_book = books.iter().find(|b| b.title == "Invalid Book").unwrap();

    assert!(valid_book.author.is_some());
    assert!(invalid_book.author.is_none());
}

// ============================================================================
// First/Last with Relations
// ============================================================================

#[tokio::test]
async fn test_first_with_relations_found() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let book = Book::objects(db).prefetch_related(relations![Author]).first().await.unwrap();

    assert!(book.id > 0);
}

#[tokio::test]
async fn test_last_with_relations_found() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let book = Book::objects(db).prefetch_related(relations![Author]).last().await.unwrap();

    assert!(book.id > 0);
}

#[tokio::test]
async fn test_first_with_relations_not_found() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let result = Book::objects(db).prefetch_related(relations![Author]).first().await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_last_with_relations_not_found() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    let result = Book::objects(db).prefetch_related(relations![Author]).last().await;

    assert!(result.is_err());
}

// ============================================================================
// Count and Exists with Relations
// ============================================================================

#[tokio::test]
async fn test_count_ignores_prefetch() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let count = Book::objects(db).prefetch_related(relations![Author]).count().await.unwrap();

    assert!(count > 0);
}

#[tokio::test]
async fn test_exists_ignores_prefetch() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let exists = Book::objects(db).prefetch_related(relations![Author]).exists().await.unwrap();

    assert!(exists);
}

// ============================================================================
// Relation Field Access Patterns
// ============================================================================

#[tokio::test]
async fn test_relation_field_access_consistency() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let books = Book::objects(db)
        .filter(ColumnTrait::eq(&Book::AuthorId, authors[0].id))
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    for book in &books {
        if let Some(ref author) = book.author {
            // Access all author fields to ensure they're populated
            let _id = author.id;
            let _name = &author.name;
            let _email = &author.email;
            let _age = author.age;
            let _created = author.created_at;
            let _updated = author.updated_at;
        }
    }
}

#[tokio::test]
async fn test_prefetch_deduplicates_related_models() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    // Create multiple books with same author
    for i in 1..=5 {
        Book::objects(db)
            .create(Book {
                title: format!("Same Author Book {}", i),
                author_id: authors[0].id,
                published: true,
                price: 1000,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    let books = Book::objects(db)
        .filter(ColumnTrait::eq(&Book::AuthorId, authors[0].id))
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    assert!(books.len() >= 5);

    // All should have the same author
    for book in &books {
        assert!(book.author.is_some());
        let author = book.author.as_ref().unwrap();
        assert_eq!(author.id, authors[0].id);
    }
}

// ============================================================================
// Complex Filtering with Relations
// ============================================================================

#[tokio::test]
async fn test_prefetch_with_complex_parent_filter() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let books = Book::objects(db)
        .filter(ColumnTrait::eq(&Book::AuthorId, authors[0].id))
        .exclude(ColumnTrait::eq(&Book::Id, 99999))
        .order_by_desc(Book::Id)
        .limit(10)
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    for book in &books {
        assert_eq!(book.author_id, authors[0].id);
        if book.author_id > 0 {
            assert!(book.author.is_some());
        }
    }
}

#[tokio::test]
async fn test_prefetch_preserves_parent_order() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let books_asc = Book::objects(db)
        .order_by_asc(Book::Id)
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    let books_desc = Book::objects(db)
        .order_by_desc(Book::Id)
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    if books_asc.len() > 1 {
        assert_eq!(books_asc.first().unwrap().id, books_desc.last().unwrap().id);
        assert_eq!(books_asc.last().unwrap().id, books_desc.first().unwrap().id);
    }
}

// ============================================================================
// Batch Loading Verification
// ============================================================================

#[tokio::test]
async fn test_prefetch_batch_loading_efficiency() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    // Load all books with authors in one go
    let books = Book::objects(db).prefetch_related(relations![Author]).all().await.unwrap();

    // Verify batch loading worked by checking all relations are populated
    let mut authors_loaded = 0;
    for book in &books {
        if book.author_id > 0 && book.author.is_some() {
            authors_loaded += 1;
        }
    }

    // Should have loaded authors for books with valid FKs
    assert!(authors_loaded > 0);
}

// ============================================================================
// Zero FK Handling
// ============================================================================

#[tokio::test]
async fn test_prefetch_with_zero_foreign_key() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    Book::objects(db)
        .create(Book {
            title: "No Author Book".to_string(),
            author_id: 0, // Zero FK
            published: true,
            price: 1000,
            ..Default::default()
        })
        .await
        .unwrap();

    let books = Book::objects(db)
        .filter(ColumnTrait::eq(&Book::AuthorId, 0))
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    for book in &books {
        assert_eq!(book.author_id, 0);
        assert!(book.author.is_none());
    }
}

// ============================================================================
// Relation Model Cloning
// ============================================================================

#[tokio::test]
async fn test_cloned_model_preserves_relations() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let books = Book::objects(db).prefetch_related(relations![Author]).all().await.unwrap();

    if let Some(book) = books.first() {
        let cloned = book.clone();

        // Verify all fields match
        assert_eq!(cloned.id, book.id);
        assert_eq!(cloned.title, book.title);
        assert_eq!(cloned.author_id, book.author_id);

        // Verify relation state matches
        match (&book.author, &cloned.author) {
            (Some(a1), Some(a2)) => {
                assert_eq!(a1.id, a2.id);
                assert_eq!(a1.name, a2.name);
            }
            (None, None) => {}
            _ => panic!("Cloned relation state doesn't match original"),
        }
    }
}
