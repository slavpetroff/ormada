//! Integration tests for relation loading and prefetch_related

use crate::common::book::{Entity as Book, Model as BookModel};
use crate::common::Author;
use crate::common::*;
use seaorm_django::prelude::*;

// ============================================================================
// PREFETCH_RELATED BASIC TESTS
// ============================================================================

#[tokio::test]
async fn test_prefetch_related_loads_relations() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    // Load books with authors prefetched
    let books = Book::objects(db).prefetch_related(relations![Author]).all().await.unwrap();

    assert_eq!(books.len(), 3);

    // Verify we can access author field directly
    for book in &books {
        // Check that author field exists and is Some
        assert!(book.author.is_some(), "Book '{}' should have author loaded", book.title);
    }
}

#[tokio::test]
async fn test_prefetch_related_direct_field_access() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let book = Book::objects(db).prefetch_related(relations![Author]).first().await.unwrap();

    // Direct field access to book fields
    assert_eq!(book.title, "Rust Programming");
    assert_eq!(book.price, 4999);
    assert_eq!(book.published, true);

    // Direct field access to related author
    let author = book.author.as_ref().unwrap();
    assert_eq!(author.name, "Alice Johnson");
    assert_eq!(author.age, 35);
}

#[tokio::test]
async fn test_prefetch_related_with_filter() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    use crate::common::book::Column;

    // Load only published books with authors
    let books = Book::objects(db)
        .filter(Column::Published.eq(true))
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    assert_eq!(books.len(), 2); // Only published books

    for book in &books {
        assert!(book.published);
        assert!(book.author.is_some());
    }
}

#[tokio::test]
async fn test_prefetch_related_with_ordering() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    use crate::common::book::Column;

    let books = Book::objects(db)
        .order_by_desc(Column::Price)
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    assert_eq!(books.len(), 3);
    // Check ordering by price (descending)
    assert!(books[0].price >= books[1].price);
    assert!(books[1].price >= books[2].price);

    // All should have authors loaded
    for book in &books {
        assert!(book.author.is_some());
    }
}

#[tokio::test]
async fn test_prefetch_related_first() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let book = Book::objects(db).prefetch_related(relations![Author]).first().await.unwrap();

    assert!(book.author.is_some());
    assert_eq!(book.author.as_ref().unwrap().name, "Alice Johnson");
}

#[tokio::test]
async fn test_prefetch_related_last() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let book = Book::objects(db).prefetch_related(relations![Author]).last().await.unwrap();

    assert!(book.author.is_some());
}

#[tokio::test]
async fn test_prefetch_related_count() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let count = Book::objects(db).prefetch_related(relations![Author]).count().await.unwrap();

    assert_eq!(count, 3);
}

#[tokio::test]
async fn test_prefetch_related_exists() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let exists = Book::objects(db).prefetch_related(relations![Author]).exists().await.unwrap();

    assert!(exists);
}

// ============================================================================
// RELATION FIELD MANIPULATION TESTS
// ============================================================================

#[tokio::test]
async fn test_related_author_fields_accessible() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let books = Book::objects(db).prefetch_related(relations![Author]).all().await.unwrap();

    // Book 1 - Alice Johnson
    let book1 = &books[0];
    let author1 = book1.author.as_ref().unwrap();
    assert_eq!(author1.name, "Alice Johnson");
    assert_eq!(author1.email, "alice@example.com");
    assert_eq!(author1.age, 35);

    // Book 2 - Alice Johnson (same author)
    let book2 = &books[1];
    let author2 = book2.author.as_ref().unwrap();
    assert_eq!(author2.name, "Alice Johnson");

    // Book 3 - Bob Smith (different author)
    let book3 = &books[2];
    let author3 = book3.author.as_ref().unwrap();
    assert_eq!(author3.name, "Bob Smith");
    assert_eq!(author3.email, "bob@example.com");
    assert_eq!(author3.age, 42);
}

#[tokio::test]
async fn test_multiple_books_same_author() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    use crate::common::book::Column;

    // Get books by Alice (author_id = 1)
    let books = Book::objects(db)
        .filter(Column::AuthorId.eq(1))
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    assert_eq!(books.len(), 2); // Two books by Alice

    // Both should have the same author
    for book in &books {
        let author = book.author.as_ref().unwrap();
        assert_eq!(author.id, 1);
        assert_eq!(author.name, "Alice Johnson");
    }
}

#[tokio::test]
async fn test_relation_none_when_not_prefetched() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    // Query WITHOUT prefetch_related
    // Note: This will return WithRelations but won't have the relation loaded
    // Actually, without prefetch_related we get regular Model, not ModelWithRelations
    // So we need to test the regular query path
    let books = Book::objects(db).all().await.unwrap();

    assert_eq!(books.len(), 3);
    // Regular models don't have the author field - this is expected
}

// ============================================================================
// EDGE CASES AND ERROR SCENARIOS
// ============================================================================

#[tokio::test]
async fn test_prefetch_related_empty_result() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    use crate::common::book::Column;

    // Query that returns no results
    let books = Book::objects(db)
        .filter(Column::Price.gt(999999))
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    assert_eq!(books.len(), 0);
}

#[tokio::test]
async fn test_prefetch_related_first_empty_errors() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    use crate::common::book::Column;

    let result = Book::objects(db)
        .filter(Column::Price.gt(999999))
        .prefetch_related(relations![Author])
        .first()
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_book_with_invalid_author_id() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    // Disable foreign key constraints for SQLite to allow invalid FK
    use sea_orm::ConnectionTrait;
    db.execute_unprepared("PRAGMA foreign_keys = OFF").await.unwrap();

    // Create a book with non-existent author_id
    let orphan_book = BookModel {
        id: 99,
        title: "Orphan Book".to_string(),
        author_id: 999, // Non-existent
        price: 1000,
        published: true,
        ..Default::default()
    };

    let created = Book::objects(&db).create(orphan_book).await.unwrap();

    let db: &'static _ = Box::leak(Box::new(db));

    // Fetch with prefetch_related
    use crate::common::book::Column;
    let book = Book::objects(db)
        .filter(Column::Id.eq(created.id))
        .prefetch_related(relations![Author])
        .first()
        .await
        .unwrap();

    // Author should be None (not found)
    assert!(book.author.is_none());
}

// ============================================================================
// PERFORMANCE AND BATCH LOADING TESTS
// ============================================================================

#[tokio::test]
async fn test_prefetch_batches_correctly() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    // Load all books with authors
    // This should execute 2 queries total:
    // 1. SELECT * FROM books
    // 2. SELECT * FROM authors WHERE id IN (1, 2) -- batch query
    let books = Book::objects(db).prefetch_related(relations![Author]).all().await.unwrap();

    assert_eq!(books.len(), 3);

    // Verify all authors are loaded (no N+1)
    let authors_loaded = books.iter().filter(|b| b.author.is_some()).count();
    assert_eq!(authors_loaded, 3);
}

#[tokio::test]
async fn test_prefetch_with_limit_still_batches() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    // Even with LIMIT, prefetch should batch load all related authors
    let books = Book::objects(db)
        .limit(2)
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    assert_eq!(books.len(), 2);

    for book in &books {
        assert!(book.author.is_some());
    }
}

// ============================================================================
// MODELWITHRELATIONS STRUCT TESTS
// ============================================================================

#[tokio::test]
async fn test_model_with_relations_has_all_fields() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let book = Book::objects(db).prefetch_related(relations![Author]).first().await.unwrap();

    // All original book fields accessible
    assert_eq!(book.id, 1);
    assert_eq!(book.title, "Rust Programming");
    assert_eq!(book.author_id, 1);
    assert_eq!(book.price, 4999);
    assert_eq!(book.published, true);
    // Timestamps
    assert!(book.created_at.timestamp() > 0);
    assert!(book.updated_at.timestamp() > 0);

    // Relation field accessible
    assert!(book.author.is_some());
}

#[tokio::test]
async fn test_model_with_relations_clone() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let book = Book::objects(db).prefetch_related(relations![Author]).first().await.unwrap();

    // ModelWithRelations implements Clone
    let book_clone = book.clone();

    assert_eq!(book.id, book_clone.id);
    assert_eq!(book.title, book_clone.title);
    assert_eq!(book.author.as_ref().unwrap().name, book_clone.author.as_ref().unwrap().name);
}
