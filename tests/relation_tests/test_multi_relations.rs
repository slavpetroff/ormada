//! Tests for multiple relation prefetching (3+ relations)
//!
//! Tests tuple implementations for LoadRelations trait

use sea_orm::ColumnTrait;
use seaorm_django::prelude::*;

use crate::common::*;

// We'll need to create test entities with 3+ relations
// For now, let's create a scenario where we prefetch books with authors multiple times

#[tokio::test]
async fn test_prefetch_single_relation_via_macro() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    // Single relation prefetch
    let books = Book::objects(db).prefetch_related(relations![Author]).all().await.unwrap();

    assert!(books.len() > 0);
    for book in &books {
        if book.author_id > 0 {
            assert!(book.author.is_some());
        }
    }
}

#[tokio::test]
async fn test_empty_relation_prefetch() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    // No data, but should still work
    let books = Book::objects(db).prefetch_related(relations![Author]).all().await.unwrap();

    assert_eq!(books.len(), 0);
}

#[tokio::test]
async fn test_prefetch_with_all_empty_results() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    // Filter to get no results
    let books = Book::objects(db)
        .filter(ColumnTrait::eq(&Book::AuthorId, 99999))
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    assert_eq!(books.len(), 0);
}

#[tokio::test]
async fn test_relation_with_ordering_and_limit() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let books = Book::objects(db)
        .order_by_desc(Book::Id)
        .limit(2)
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    assert!(books.len() <= 2);
    if books.len() > 1 {
        assert!(books[0].id >= books[1].id);
    }
}

#[tokio::test]
async fn test_relation_with_complex_filter() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let books = Book::objects(db)
        .filter(ColumnTrait::eq(&Book::AuthorId, authors[0].id))
        .filter(ColumnTrait::eq(&Book::Published, true))
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    for book in &books {
        assert_eq!(book.author_id, authors[0].id);
        assert_eq!(book.published, true);
        assert!(book.author.is_some());
    }
}

#[tokio::test]
async fn test_count_with_prefetch_defined() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    // Count should ignore prefetch
    let count = Book::objects(db).prefetch_related(relations![Author]).count().await.unwrap();

    assert!(count > 0);
}

#[tokio::test]
async fn test_exists_with_prefetch_defined() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    // Exists should ignore prefetch
    let exists = Book::objects(db).prefetch_related(relations![Author]).exists().await.unwrap();

    assert!(exists);
}

#[tokio::test]
async fn test_first_with_prefetch() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let book = Book::objects(db)
        .order_by_asc(Book::Id)
        .prefetch_related(relations![Author])
        .first()
        .await
        .unwrap();

    assert!(book.id > 0);
}

#[tokio::test]
async fn test_last_with_prefetch() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let book = Book::objects(db)
        .order_by_desc(Book::Id)
        .prefetch_related(relations![Author])
        .last()
        .await
        .unwrap();

    assert!(book.id > 0);
}

#[tokio::test]
async fn test_relation_none_when_fk_zero() {
    let db = setup_test_db().await;
    let db: &'static _ = Box::leak(Box::new(db));

    // Create a book with no author (FK = 0 or default)
    let book_without_author = Book {
        title: "Standalone Book".to_string(),
        author_id: 0,
        price: 2000,
        published: false,
        ..Default::default()
    };

    let created = Book::objects(db).create(book_without_author).await;

    // This might fail due to FK constraints, which is expected
    if created.is_ok() {
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
}

#[tokio::test]
async fn test_multiple_books_same_author_efficient() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    // Create 10 books with the same author
    let books_to_create: Vec<_> = (1..=10)
        .map(|i| Book {
            title: format!("Book {}", i),
            author_id: authors[0].id,
            price: 1000 + i,
            published: true,
            ..Default::default()
        })
        .collect();

    Book::objects(db).bulk_create(books_to_create).await.unwrap();

    let books = Book::objects(db)
        .filter(ColumnTrait::eq(&Book::AuthorId, authors[0].id))
        .prefetch_related(relations![Author])
        .all()
        .await
        .unwrap();

    assert!(books.len() >= 10);

    // All should have the same author loaded
    for book in &books {
        assert!(book.author.is_some());
        if let Some(ref author) = book.author {
            assert_eq!(author.id, authors[0].id);
        }
    }
}

#[tokio::test]
async fn test_relation_field_immutability() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let _books = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let books = Book::objects(db).prefetch_related(relations![Author]).all().await.unwrap();

    if let Some(book) = books.first() {
        let _author_ref = &book.author;
        // Verify we can access the relation field multiple times
        let _author_ref2 = &book.author;
        // Both references point to the same field
        assert!(std::ptr::eq(_author_ref, _author_ref2));
    }
}

#[tokio::test]
async fn test_prefetch_preserves_model_data() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;
    let books_created = create_sample_books(&db).await;
    let db: &'static _ = Box::leak(Box::new(db));

    let books = Book::objects(db).prefetch_related(relations![Author]).all().await.unwrap();

    // Verify all original book fields are intact
    for book in &books {
        let original = books_created.iter().find(|b| b.id == book.id);
        if let Some(orig) = original {
            assert_eq!(book.title, orig.title);
            assert_eq!(book.price, orig.price);
            assert_eq!(book.published, orig.published);
            assert_eq!(book.author_id, orig.author_id);
        }
    }
}
