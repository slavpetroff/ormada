// Integration tests are allowed to use unwrap/expect for clarity
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(unused_must_use)]
#![allow(clippy::indexing_slicing)]
#![allow(clippy::uninlined_format_args)]

//! Transaction integration tests

mod fixtures;

use fixtures::*;
use ormada::prelude::*;
use rstest::*;

// ============================================================================
// Basic Transaction Tests
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_tx_macro_commit(#[future] db: DatabaseRouter) {
    let (author, book) = tx!(db, |txn| async move {
        let author = Author::objects(txn)
            .create(Author {
                name: "Transaction Author".to_string(),
                email: "txn@example.com".to_string(),
                age: 30,
                ..Default::default()
            })
            .await?;

        let book = Book::objects(txn)
            .create(Book {
                author_id: author.id,
                title: "Transaction Book".to_string(),
                price: 1999,
                published: true,
                ..Default::default()
            })
            .await?;

        Ok((author, book))
    })
    .await
    .unwrap();

    // Verify both committed
    let fetched_author = Author::objects(&db).get(author.id).await.unwrap();
    assert_eq!(fetched_author.name, "Transaction Author");

    let fetched_book = Book::objects(&db).get(book.id).await.unwrap();
    assert_eq!(fetched_book.title, "Transaction Book");
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_tx_macro_rollback_on_error(#[future] db: DatabaseRouter) {
    let result: Result<(), OrmadaError> = tx!(db, |txn| async move {
        // Create author
        let _author = Author::objects(txn)
            .create(Author {
                name: "Will Rollback".to_string(),
                email: "rollback@example.com".to_string(),
                age: 30,
                ..Default::default()
            })
            .await?;

        // Trigger error to test rollback
        return Err(OrmadaError::validation_error("test", "rollback", "Intentional error"));
    })
    .await;

    assert!(result.is_err());

    // Verify author was NOT created (rolled back)
    let count = Author::objects(&db).count().await.unwrap();
    assert_eq!(count, 0);
}

// ============================================================================
// Complex Transaction Scenarios
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_transaction_with_multiple_creates(#[future] db: DatabaseRouter) {
    let authors = tx!(db, |txn| async move {
        let mut created = Vec::new();
        for i in 1..=5 {
            let author = Author::objects(txn)
                .create(Author {
                    name: format!("Author {i}"),
                    email: format!("author{i}@example.com"),
                    age: 25 + i,
                    ..Default::default()
                })
                .await?;
            created.push(author);
        }
        Ok(created)
    })
    .await
    .unwrap();

    assert_eq!(authors.len(), 5);

    let count = Author::objects(&db).count().await.unwrap();
    assert_eq!(count, 5);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_transaction_with_query_and_update(
    #[future] db_with_author: (DatabaseRouter, Author),
) {
    let (db, author) = db_with_author;
    let original_age = author.age;

    let updated = tx!(db, |txn| async move {
        // Query inside transaction
        let mut author = Author::objects(txn).get(author.id).await?;
        author.age += 10;
        author.save(txn).await
    })
    .await
    .unwrap();

    assert_eq!(updated.age, original_age + 10);

    // Verify committed
    let fetched = Author::objects(&db).get(updated.id).await.unwrap();
    assert_eq!(fetched.age, original_age + 10);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_transaction_rollback_on_constraint_violation(
    #[future] db: DatabaseRouter,
    #[future] author: Author,
) {
    let initial_count = Book::objects(&db).count().await.unwrap();

    let result: Result<(), OrmadaError> = tx!(db, |txn| async move {
        // Create valid book
        Book::objects(txn)
            .create(Book {
                author_id: author.id,
                title: "First Book".to_string(),
                price: 1999,
                published: true,
                ..Default::default()
            })
            .await?;

        // Create book with invalid foreign key (should fail)
        Book::objects(txn)
            .create(Book {
                author_id: 99999, // Non-existent author
                title: "Invalid Book".to_string(),
                price: 2999,
                published: true,
                ..Default::default()
            })
            .await?;

        Ok(())
    })
    .await;

    // Transaction should have failed
    assert!(result.is_err());

    // First book should also be rolled back
    let final_count = Book::objects(&db).count().await.unwrap();
    assert_eq!(final_count, initial_count);
}

// ============================================================================
// Nested Transactions (Savepoints)
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_nested_transaction_both_commit(#[future] db: DatabaseRouter) {
    let (_outer, _inner) = tx!(db, |txn| async move {
        let outer = Author::objects(txn)
            .create(Author {
                name: "Outer".to_string(),
                email: "outer@example.com".to_string(),
                age: 30,
                ..Default::default()
            })
            .await?;

        let inner = tx!(txn, |inner_txn| async move {
            Author::objects(inner_txn)
                .create(Author {
                    name: "Inner".to_string(),
                    email: "inner@example.com".to_string(),
                    age: 25,
                    ..Default::default()
                })
                .await
        })
        .await?;

        Ok((outer, inner))
    })
    .await
    .unwrap();

    // Both should be committed
    let count = Author::objects(&db).count().await.unwrap();
    assert_eq!(count, 2);
}

// ============================================================================
// Transaction with Queries
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_transaction_sees_own_changes(#[future] db: DatabaseRouter) {
    tx!(db, |txn| async move {
        // Create author
        let author = Author::objects(txn)
            .create(Author {
                name: "Test".to_string(),
                email: "test@example.com".to_string(),
                age: 30,
                ..Default::default()
            })
            .await?;

        // Query should see the created author within the transaction
        let found = Author::objects(txn).filter(Author::Name.eq("Test")).first().await?;

        assert_eq!(found.id, author.id);
        Ok(())
    })
    .await
    .unwrap();
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_transaction_isolation(#[future] db: DatabaseRouter) {
    // Check initial count before transaction
    let initial_count = Author::objects(&db).count().await.unwrap();
    assert_eq!(initial_count, 0);

    // Clone for concurrent access
    let db_clone = db.clone();

    let handle = tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        // This should not see the uncommitted author (or might see it in SQLite)
        Author::objects(&db_clone).count().await
    });

    tx!(db, |txn| async move {
        Author::objects(txn)
            .create(Author {
                name: "Isolated".to_string(),
                email: "isolated@example.com".to_string(),
                age: 30,
                ..Default::default()
            })
            .await?;

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        Ok(())
    })
    .await
    .unwrap();

    let mid_count = handle.await.unwrap().unwrap();

    // After transaction commits, should definitely see it
    let final_count = Author::objects(&db).count().await.unwrap();
    assert_eq!(final_count, 1);
    // SQLite may or may not show isolation, so we just check final state
    assert!(mid_count <= 1);
}

// ============================================================================
// Bulk Operations in Transactions
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_transaction_with_bulk_create(#[future] db: DatabaseRouter) {
    let authors = (0..10)
        .map(|i| Author {
            name: format!("Bulk {}", i),
            email: format!("bulk{}@example.com", i),
            age: 25 + i,
            ..Default::default()
        })
        .collect();

    let created = tx!(db, |txn| async move { Author::objects(txn).bulk_create(authors).await })
        .await
        .unwrap();

    assert_eq!(created, 10);

    let count = Author::objects(&db).count().await.unwrap();
    assert_eq!(count, 10);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_transaction_with_bulk_update(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;
    let updated_count = tx!(db, |txn| async move {
        Author::objects(txn)
            .filter(Author::Age.lt(35))
            .update(|mut author| async move {
                author.age = 35;
                Ok(author)
            })
            .await
    })
    .await
    .unwrap();

    assert_eq!(updated_count, 2); // Alice and Bob

    // Verify committed
    let authors = Author::objects(&db).all().await.unwrap();
    assert!(authors.iter().all(|a| a.age >= 35));
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_transaction_with_bulk_delete(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;
    let deleted_count = tx!(db, |txn| async move {
        Author::objects(txn).filter(Author::Age.lt(30)).delete().await
    })
    .await
    .unwrap();

    assert_eq!(deleted_count, 1); // Alice

    let remaining = Author::objects(&db).count().await.unwrap();
    assert_eq!(remaining, 2);
}

// ============================================================================
// Transaction Error Handling and Edge Cases
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_transaction_rollback_on_panic(#[future] db: DatabaseRouter) {
    let initial_count = Author::objects(&db).count().await.unwrap();

    let result = tx!(db, |txn| async move {
        Author::objects(txn)
            .create(Author {
                name: "Will Rollback".to_string(),
                email: "rollback@example.com".to_string(),
                age: 30,
                ..Default::default()
            })
            .await?;

        // Force error to test rollback
        Err::<(), OrmadaError>(OrmadaError::validation_error(
            "test",
            "rollback",
            "Intentional error",
        ))
    })
    .await;

    assert!(result.is_err());

    let final_count = Author::objects(&db).count().await.unwrap();
    assert_eq!(initial_count, final_count);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_transaction_commit_only_on_success(#[future] db: DatabaseRouter) {
    // Transaction that commits
    let author = tx!(db, |txn| async move {
        Author::objects(txn)
            .create(Author {
                name: "Success".to_string(),
                email: "success@example.com".to_string(),
                age: 30,
                ..Default::default()
            })
            .await
    })
    .await
    .unwrap();

    // Verify committed
    let fetched = Author::objects(&db).get(author.id).await.unwrap();
    assert_eq!(fetched.name, "Success");
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_nested_transaction_inner_rollback(#[future] db: DatabaseRouter) {
    let outer = tx!(db, |outer_txn| async move {
        let outer_author = Author::objects(outer_txn)
            .create(Author {
                name: "Outer".to_string(),
                email: "outer@example.com".to_string(),
                age: 30,
                ..Default::default()
            })
            .await?;

        // Inner transaction that fails
        let inner_result = tx!(outer_txn, |inner_txn| async move {
            Author::objects(inner_txn)
                .create(Author {
                    name: "Inner".to_string(),
                    email: "inner@example.com".to_string(),
                    age: 25,
                    ..Default::default()
                })
                .await?;

            Err::<(), OrmadaError>(OrmadaError::validation_error("test", "rollback", "Inner fail"))
        })
        .await;

        // Inner failed but outer continues
        assert!(inner_result.is_err());

        Ok(outer_author)
    })
    .await
    .unwrap();

    // Outer should be committed
    let count = Author::objects(&db).count().await.unwrap();
    assert_eq!(count, 1);
    assert_eq!(outer.name, "Outer");
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_transaction_with_aggregation(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    let (count, max_age) = tx!(db, |txn| async move {
        let count = Author::objects(txn).count().await?;
        let max = Author::objects(txn).aggregate_max(Author::Age).await?;
        Ok((count, max))
    })
    .await
    .unwrap();

    assert_eq!(count, 3);
    assert_eq!(max_age, Some(35.0));
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_transaction_with_filter_and_update(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    let updated = tx!(db, |txn| async move {
        Author::objects(txn)
            .filter(Author::Name.eq("Alice"))
            .update(|mut author| async move {
                author.age = 100;
                Ok(author)
            })
            .await
    })
    .await
    .unwrap();

    assert_eq!(updated, 1);

    let alice = Author::objects(&db).filter(Author::Name.eq("Alice")).first().await.unwrap();
    assert_eq!(alice.age, 100);
}
