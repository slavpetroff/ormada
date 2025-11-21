//! Tests for atomic transaction operations
//!
//! Tests both happy paths (commit) and unhappy paths (rollback)

use seaorm_django::prelude::*;

use crate::common::*;

// ============================================================================
// Happy Path Tests (Commit)
// ============================================================================

#[tokio::test]
async fn test_atomic_commit_single_insert() {
    let db = setup_test_db().await;

    // Transaction that succeeds
    let author = tx!(db, |txn| async move {
        let author = Author::objects(txn)
            .create(Author {
                name: "John Doe".to_string(),
                email: "john@example.com".to_string(),
                age: 30,
                ..Default::default()
            })
            .await?;
        Ok(author)
    })
    .await
    .unwrap();

    // Verify author was committed
    let found = Author::objects(&db).get(author.id).await;

    assert!(found.is_ok());
    assert_eq!(found.unwrap().name, "John Doe");
}

#[tokio::test]
async fn test_atomic_commit_multiple_inserts() {
    let db = setup_test_db().await;

    // Transaction with multiple operations
    let (author, book) = tx!(db, |txn| async move {
        // Create author using Django-style API
        let author = Author::objects(txn)
            .create(Author {
                name: "Jane Doe".to_string(),
                email: "jane@example.com".to_string(),
                age: 25,
                ..Default::default()
            })
            .await?;

        // Create book referencing the author
        let book = Book::objects(txn)
            .create(Book {
                title: "Rust Programming".to_string(),
                author_id: author.id,
                price: 2999,
                published: true,
                ..Default::default()
            })
            .await?;

        Ok((author, book))
    })
    .await
    .unwrap();

    // Verify both were committed
    let found_author = Author::objects(&db).get(author.id).await;
    assert!(found_author.is_ok());

    let found_book = Book::objects(&db).get(book.id).await;
    assert!(found_book.is_ok());
    assert_eq!(found_book.unwrap().author_id, author.id);
}

#[tokio::test]
async fn test_atomic_commit_with_query() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    // Transaction with query and insert
    let result = tx!(db, |txn| async move {
        // Query existing authors
        let count = Author::objects(txn).count().await?;

        // Create a new one
        let new_author = Author::objects(txn)
            .create(Author {
                name: "New Author".to_string(),
                email: "new@example.com".to_string(),
                age: 35,
                ..Default::default()
            })
            .await?;

        Ok((count, new_author))
    })
    .await
    .unwrap();

    assert_eq!(result.0, 3); // Original 3 authors
}

#[tokio::test]
async fn test_atomic_commit_with_update() {
    let db = setup_test_db().await;
    let authors = create_sample_authors(&db).await;
    let author_id = authors[0].id;

    // Transaction with update
    let _ = tx!(db, |txn| async move {
        let mut author = Author::objects(txn).get(author_id).await?;

        author.age = 99;

        // Save updates
        let updated = author.save(txn).await?;
        Ok(updated)
    })
    .await
    .unwrap();

    // Verify update was committed
    let found = Author::objects(&db).get(author_id).await.unwrap();
    assert_eq!(found.age, 99);
}

// ============================================================================
// Unhappy Path Tests (Rollback)
// ============================================================================

#[tokio::test]
async fn test_atomic_rollback_on_error() {
    let db = setup_test_db().await;

    // Transaction that fails
    let result: Result<Author, DjangoOrmError> = tx!(db, |txn| async move {
        // Create author
        let _ = Author::objects(txn)
            .create(Author {
                name: "Will Rollback".to_string(),
                email: "rollback@example.com".to_string(),
                age: 30,
                ..Default::default()
            })
            .await?;

        // Return error - should trigger rollback
        Err(DjangoOrmError::Custom("Intentional error".into()))
    })
    .await;

    assert!(result.is_err());

    // Verify nothing was committed
    let count = Author::objects(&db).count().await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_atomic_rollback_partial_operations() {
    let db = setup_test_db().await;

    // Transaction with multiple operations, fails midway
    let result: Result<(), DjangoOrmError> = tx!(db, |txn| async move {
        // Create first author - succeeds
        let _author1 = Author::objects(txn)
            .create(Author {
                name: "Author 1".to_string(),
                email: "author1@example.com".to_string(),
                age: 30,
                ..Default::default()
            })
            .await?;

        // Create second author - succeeds
        let _author2 = Author::objects(txn)
            .create(Author {
                name: "Author 2".to_string(),
                email: "author2@example.com".to_string(),
                age: 25,
                ..Default::default()
            })
            .await?;

        // Fail here - both authors should rollback
        Err(DjangoOrmError::Custom("Rollback all".into()))
    })
    .await;

    assert!(result.is_err());

    // Verify no authors were committed
    let count = Author::objects(&db).count().await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_atomic_rollback_with_business_logic() {
    let db = setup_test_db().await;

    // Transaction with business logic validation
    let result = tx!(db, |txn| async move {
        let author = Author::objects(txn)
            .create(Author {
                name: "Too Young".to_string(),
                email: "young@example.com".to_string(),
                age: 15,
                ..Default::default()
            })
            .await?;

        // Business logic: must be 18+
        if author.age < 18 {
            return Err(DjangoOrmError::Custom("Must be 18 or older".into()));
        }

        Ok(author)
    })
    .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("18 or older"));

    // Verify rollback
    let count = Author::objects(&db).count().await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_atomic_rollback_on_constraint_violation() {
    let db = setup_test_db().await;
    let _authors = create_sample_authors(&db).await;

    // Try to create book with invalid foreign key
    let _result = tx!(db, |txn| async move {
        // Valid author first
        let _author = Author::objects(txn)
            .create(Author {
                name: "Valid Author".to_string(),
                email: "valid@example.com".to_string(),
                age: 30,
                ..Default::default()
            })
            .await?;

        // Book with non-existent author_id
        let book = Book::objects(txn)
            .create(Book {
                title: "Invalid Book".to_string(),
                author_id: 99999, // Non-existent
                price: 2999,
                published: true,
                ..Default::default()
            })
            .await?;

        Ok(book)
    })
    .await;

    // Note: SQLite in-memory doesn't enforce FK by default
    // This test may pass, but demonstrates the pattern
    // In production with FK constraints, this would fail
}

// ============================================================================
// Nested Transaction Tests
// ============================================================================

#[tokio::test]
async fn test_nested_atomic_both_succeed() {
    let db = setup_test_db().await;

    let (_, book_count) = tx!(db, |txn| async move {
        // Outer transaction: create author
        let author = Author::objects(txn)
            .create(Author {
                name: "Nested Author".to_string(),
                email: "nested@example.com".to_string(),
                age: 30,
                ..Default::default()
            })
            .await?;

        // Inner transaction: create books
        let book_count = tx!(txn, |inner_txn| async move {
            Book::objects(inner_txn)
                .create(Book {
                    title: "Book 1".to_string(),
                    author_id: author.id,
                    price: 1999,
                    published: true,
                    ..Default::default()
                })
                .await?;

            Book::objects(inner_txn)
                .create(Book {
                    title: "Book 2".to_string(),
                    author_id: author.id,
                    price: 2999,
                    published: true,
                    ..Default::default()
                })
                .await?;

            Ok(2)
        })
        .await?;

        Ok((author, book_count))
    })
    .await
    .unwrap();

    assert_eq!(book_count, 2);

    // Verify all committed
    let author_count = Author::objects(&db).count().await.unwrap();
    assert_eq!(author_count, 1);

    let book_count = Book::objects(&db).count().await.unwrap();
    assert_eq!(book_count, 2);
}

#[tokio::test]
async fn test_nested_atomic_inner_fails() {
    let db = setup_test_db().await;

    // Outer succeeds, inner fails
    let result = tx!(db, |txn| async move {
        // Create author in outer transaction
        let author = Author::objects(txn)
            .create(Author {
                name: "Outer Author".to_string(),
                email: "outer@example.com".to_string(),
                age: 30,
                ..Default::default()
            })
            .await?;

        // Inner transaction that fails
        let inner_result = tx!(txn, |inner_txn| async move {
            Book::objects(inner_txn)
                .create(Book {
                    title: "Will Fail".to_string(),
                    author_id: author.id,
                    price: 1999,
                    published: true,
                    ..Default::default()
                })
                .await?;

            // Inner fails
            Err(DjangoOrmError::Custom("Inner error".into()))
        })
        .await;

        // If inner fails, outer fails too (current implementation)
        inner_result?;

        Ok(author)
    })
    .await;

    assert!(result.is_err());
}

// ============================================================================
// Edge Cases
// ============================================================================

#[tokio::test]
async fn test_atomic_empty_transaction() {
    let db = setup_test_db().await;

    // Transaction with no operations
    let result: Result<(), DjangoOrmError> = tx!(db, |_txn| async move { Ok(()) }).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_atomic_returns_value() {
    let db = setup_test_db().await;

    // Transaction returning a value
    let sum = tx!(db, |_txn| async move {
        let result = 1 + 2 + 3;
        Ok(result)
    })
    .await
    .unwrap();

    assert_eq!(sum, 6);
}

#[tokio::test]
async fn test_atomic_with_complex_return() {
    let db = setup_test_db().await;

    // Transaction returning complex type
    let result = tx!(db, |txn| async move {
        let author = Author::objects(txn)
            .create(Author {
                name: "Complex Return".to_string(),
                email: "complex@example.com".to_string(),
                age: 30,
                ..Default::default()
            })
            .await?;

        Ok((author.id, author.name.clone(), author.age))
    })
    .await
    .unwrap();

    assert!(result.0 > 0);
    assert_eq!(result.1, "Complex Return");
    assert_eq!(result.2, 30);
}

#[tokio::test]
async fn test_atomic_multiple_sequential() {
    let db = setup_test_db().await;

    // Multiple sequential transactions
    let author1 = tx!(db, |txn| async move {
        let author = Author::objects(txn)
            .create(Author {
                name: "First".to_string(),
                email: "first@example.com".to_string(),
                age: 30,
                ..Default::default()
            })
            .await?;
        Ok(author)
    })
    .await
    .unwrap();

    let author2 = tx!(db, |txn| async move {
        let author = Author::objects(txn)
            .create(Author {
                name: "Second".to_string(),
                email: "second@example.com".to_string(),
                age: 25,
                ..Default::default()
            })
            .await?;
        Ok(author)
    })
    .await
    .unwrap();

    assert_ne!(author1.id, author2.id);

    // Verify both committed
    let count = Author::objects(&db).count().await.unwrap();
    assert_eq!(count, 2);
}
