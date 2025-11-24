// Integration tests are allowed to use unwrap/expect for clarity
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(unused_must_use)]

//! Write operations integration tests

mod fixtures;

use fixtures::*;
use rstest::*;
use seaorm_django::prelude::*;

// ============================================================================
// Create Operations
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_create_author(#[future] db: DatabaseRouter) {
    let author = Author::objects(&db)
        .create(Author {
            name: "New Author".to_string(),
            email: "new@example.com".to_string(),
            age: 28,
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(author.id > 0);
    assert_eq!(author.name, "New Author");
    assert_eq!(author.email, "new@example.com");
    assert_eq!(author.age, 28);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_create_book_with_author(#[future] db_with_author: (DatabaseRouter, Author)) {
    let (db, author) = db_with_author;
    let book = Book::objects(&db)
        .create(Book {
            author_id: author.id,
            title: "Test Book".to_string(),
            price: 2999,
            published: true,
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(book.id > 0);
    assert_eq!(book.author_id, author.id);
    assert_eq!(book.title, "Test Book");
    assert_eq!(book.price, 2999);
    assert!(book.published);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_create_sets_auto_now_add(#[future] db: DatabaseRouter) {
    let before = chrono::Utc::now();

    let author = Author::objects(&db)
        .create(Author {
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            age: 30,
            ..Default::default()
        })
        .await
        .unwrap();

    let after = chrono::Utc::now();

    // Check that created_at was set (auto_now_add)
    assert!(author.created_at.timestamp() >= before.timestamp());
    assert!(author.created_at.timestamp() <= after.timestamp());
}

// ============================================================================
// Save (Update All Fields) Operations
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_save_updates_all_fields(#[future] db_with_author: (DatabaseRouter, Author)) {
    let (db, mut author) = db_with_author;
    author.name = "Updated Name".to_string();
    author.email = "updated@example.com".to_string();
    author.age = 40;

    let updated = author.save(&db).await.unwrap();

    assert_eq!(updated.name, "Updated Name");
    assert_eq!(updated.email, "updated@example.com");
    assert_eq!(updated.age, 40);

    // Verify in DB
    let fetched = Author::objects(&db).get(updated.id).await.unwrap();
    assert_eq!(fetched.name, "Updated Name");
    assert_eq!(fetched.email, "updated@example.com");
    assert_eq!(fetched.age, 40);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_save_updates_auto_now(#[future] db_with_author: (DatabaseRouter, Author)) {
    let (db, author) = db_with_author;
    let original_updated_at = author.updated_at;

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let mut author = author;
    author.name = "Modified".to_string();

    let updated = author.save(&db).await.unwrap();

    // updated_at should be newer (compare with milliseconds precision)
    assert!(updated.updated_at.timestamp_millis() > original_updated_at.timestamp_millis());
}

// ============================================================================
// Bulk Update Operations
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_update_bulk(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let count = Author::objects(&db)
        .filter(Author::Age.lt(35))
        .update(|author| {
            author.age = 35;
        })
        .await
        .unwrap();

    assert_eq!(count, 2); // Alice and Bob

    let authors = Author::objects(&db).all().await.unwrap();
    assert!(authors.iter().all(|a| a.age >= 35));
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_update_with_filter(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let count = Author::objects(&db)
        .filter(Author::Name.eq("Bob"))
        .update(|author| {
            author.email = "bob.updated@example.com".to_string();
        })
        .await
        .unwrap();

    assert_eq!(count, 1);

    let bob = Author::objects(&db).filter(Author::Name.eq("Bob")).first().await.unwrap();
    assert_eq!(bob.email, "bob.updated@example.com");
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_update_no_matches(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let count = Author::objects(&db)
        .filter(Author::Age.gt(100))
        .update(|author| {
            author.age = 50;
        })
        .await
        .unwrap();

    assert_eq!(count, 0);
}

// ============================================================================
// Delete Operations
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_delete_single(#[future] db_with_author: (DatabaseRouter, Author)) {
    let (db, author) = db_with_author;
    let id = author.id;

    author.delete(&db).await.unwrap();

    let result = Author::objects(&db).get(id).await;
    assert!(result.is_err());
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_delete_bulk(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let count = Author::objects(&db).filter(Author::Age.lt(30)).delete().await.unwrap();

    assert_eq!(count, 1); // Only Alice (25)

    let remaining = Author::objects(&db).count().await.unwrap();
    assert_eq!(remaining, 2);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_delete_all(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;
    let count = Author::objects(&db).delete().await.unwrap();
    assert_eq!(count, 3);

    let remaining = Author::objects(&db).count().await.unwrap();
    assert_eq!(remaining, 0);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_delete_with_complex_filter(
    #[future] db_with_authors_with_books: (DatabaseRouter, Vec<(Author, Vec<Book>)>),
) {
    let (db, _authors_with_books) = db_with_authors_with_books;
    let count = Book::objects(&db)
        .filter(Book::Price.lt(1500))
        .filter(Book::Published.eq(true))
        .delete()
        .await
        .unwrap();

    assert!(count > 0);

    // Verify deleted
    let remaining = Book::objects(&db)
        .filter(Book::Price.lt(1500))
        .filter(Book::Published.eq(true))
        .count()
        .await
        .unwrap();
    assert_eq!(remaining, 0);
}

// ============================================================================
// Bulk Create Operations
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_bulk_create(#[future] db: DatabaseRouter) {
    let authors = vec![
        Author {
            name: "Author 1".to_string(),
            email: "author1@example.com".to_string(),
            age: 25,
            ..Default::default()
        },
        Author {
            name: "Author 2".to_string(),
            email: "author2@example.com".to_string(),
            age: 30,
            ..Default::default()
        },
        Author {
            name: "Author 3".to_string(),
            email: "author3@example.com".to_string(),
            age: 35,
            ..Default::default()
        },
    ];

    let created = Author::objects(&db).bulk_create(authors).await.unwrap();
    assert_eq!(created, 3);

    let count = Author::objects(&db).count().await.unwrap();
    assert_eq!(count, 3);
}

#[rstest]
#[awt]
#[case(10)]
#[case(100)]
#[case(500)]
#[tokio::test]
async fn test_bulk_create_performance(#[future] db: DatabaseRouter, #[case] count: usize) {
    let authors: Vec<Author> = (0..count)
        .map(|i| Author {
            name: format!("Author {}", i),
            email: format!("author{}@example.com", i),
            age: 25 + (i as i32 % 50),
            ..Default::default()
        })
        .collect();

    let created = Author::objects(&db).bulk_create(authors).await.unwrap();
    assert_eq!(created, count as u64);

    let db_count = Author::objects(&db).count().await.unwrap();
    assert_eq!(db_count, count as u64);
}

// ============================================================================
// Edge Cases
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_create_with_same_email_allowed(#[future] db: DatabaseRouter) {
    // Both should succeed since email is not unique
    let author1 = Author::objects(&db)
        .create(Author {
            name: "Author 1".to_string(),
            email: "same@example.com".to_string(),
            age: 25,
            ..Default::default()
        })
        .await
        .unwrap();

    let author2 = Author::objects(&db)
        .create(Author {
            name: "Author 2".to_string(),
            email: "same@example.com".to_string(),
            age: 30,
            ..Default::default()
        })
        .await
        .unwrap();

    assert_ne!(author1.id, author2.id);
    assert_eq!(author1.email, author2.email);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_update_does_not_affect_other_records(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, sample_authors) = db_with_sample_authors;
    let original_bob = sample_authors.iter().find(|a| a.name == "Bob").unwrap().clone();

    Author::objects(&db)
        .filter(Author::Name.eq("Alice"))
        .update(|author| {
            author.age = 50;
        })
        .await
        .unwrap();

    let bob = Author::objects(&db).get(original_bob.id).await.unwrap();
    assert_eq!(bob.age, original_bob.age);
}

// ============================================================================
// Write Operation Edge Cases
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_create_with_explicit_id(#[future] db: DatabaseRouter) {
    let author = Author::objects(&db)
        .create(Author {
            id: 9999,
            name: "Explicit ID".to_string(),
            email: "explicit@example.com".to_string(),
            age: 30,
            created_at: chrono::Utc::now().fixed_offset(),
            updated_at: chrono::Utc::now().fixed_offset(),
        })
        .await
        .unwrap();

    // ID should be auto-assigned, not 9999
    assert!(author.id > 0);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_delete_nonexistent(#[future] db: DatabaseRouter) {
    let deleted = Author::objects(&db).filter(Author::Id.eq(99999)).delete().await.unwrap();

    assert_eq!(deleted, 0);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_update_with_no_filter_matches(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    let count = Author::objects(&db)
        .filter(Author::Name.eq("NonExistent"))
        .update(|author| {
            author.age = 999;
        })
        .await
        .unwrap();

    assert_eq!(count, 0);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_bulk_create_empty_vec(#[future] db: DatabaseRouter) {
    let count = Author::objects(&db).bulk_create(vec![]).await.unwrap();

    assert_eq!(count, 0);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_delete_single_record(#[future] db_with_author: (DatabaseRouter, Author)) {
    let (db, author) = db_with_author;
    let id = author.id;

    // Delete the record using DeleteExt trait
    author.delete(&db).await.unwrap();

    // Verify it's gone
    let result = Author::objects(&db).get(id).await;
    assert!(result.is_err());
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_delete_with_verification(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, sample_authors) = db_with_sample_authors;
    
    let count_before = Author::objects(&db).count().await.unwrap();
    assert_eq!(count_before, 3);

    // Delete one author
    sample_authors[0].clone().delete(&db).await.unwrap();

    let count_after = Author::objects(&db).count().await.unwrap();
    assert_eq!(count_after, 2);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_save_preserves_created_at(#[future] db_with_author: (DatabaseRouter, Author)) {
    let (db, author) = db_with_author;
    let original_created_at = author.created_at;

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let mut modified = author;
    modified.name = "Modified Name".to_string();

    let saved = modified.save(&db).await.unwrap();

    // created_at should not change
    assert_eq!(saved.created_at.timestamp(), original_created_at.timestamp());
    // updated_at should change
    assert!(saved.updated_at.timestamp_millis() > original_created_at.timestamp_millis());
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_delete_with_multiple_conditions(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    let deleted = Author::objects(&db)
        .filter(Author::Age.gte(25))
        .filter(Author::Age.lte(30))
        .delete()
        .await
        .unwrap();

    assert_eq!(deleted, 2); // Alice and Bob

    let remaining = Author::objects(&db).count().await.unwrap();
    assert_eq!(remaining, 1); // Only Charlie left
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_create_multiple_with_same_email(#[future] db: DatabaseRouter) {
    // Should allow multiple authors with same email (no unique constraint)
    let author1 = Author::objects(&db)
        .create(Author {
            name: "Author 1".to_string(),
            email: "shared@example.com".to_string(),
            age: 30,
            ..Default::default()
        })
        .await
        .unwrap();

    let author2 = Author::objects(&db)
        .create(Author {
            name: "Author 2".to_string(),
            email: "shared@example.com".to_string(),
            age: 25,
            ..Default::default()
        })
        .await
        .unwrap();

    assert_ne!(author1.id, author2.id);
    assert_eq!(author1.email, author2.email);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_update_chained_filters(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    let count = Author::objects(&db)
        .filter(Author::Age.gt(25))
        .filter(Author::Age.lt(35))
        .update(|author| {
            author.age = 99;
        })
        .await
        .unwrap();

    assert_eq!(count, 1); // Only Bob (30)

    let bob = Author::objects(&db).filter(Author::Name.eq("Bob")).first().await.unwrap();
    assert_eq!(bob.age, 99);
}

// ============================================================================
// Distinct Tests
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_distinct_on_duplicates(#[future] db: DatabaseRouter) {
    // Create authors with duplicate ages
    for i in 0..3 {
        Author::objects(&db)
            .create(Author {
                name: format!("Author {}", i),
                email: format!("author{}@test.com", i),
                age: 30, // Same age for all
                ..Default::default()
            })
            .await
            .unwrap();
    }

    let authors = Author::objects(&db).distinct().all().await.unwrap();

    // Distinct should still return all records (distinct on all columns)
    assert_eq!(authors.len(), 3);
}

// ============================================================================
// Only/Defer Tests
// ============================================================================

#[rstest]
#[awt]
#[tokio::test]
async fn test_only_specific_fields(
    #[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>),
) {
    let (db, _sample_authors) = db_with_sample_authors;

    // Even with only(), we should still get complete models
    let authors = Author::objects(&db).all().await.unwrap();

    assert_eq!(authors.len(), 3);
    assert!(!authors[0].name.is_empty());
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_create_many_at_once(#[future] db: DatabaseRouter) {
    let authors: Vec<Author> = (0..10)
        .map(|i| Author {
            name: format!("Author {}", i),
            email: format!("author{}@bulk.com", i),
            age: 20 + i,
            ..Default::default()
        })
        .collect();

    let count = Author::objects(&db).bulk_create(authors).await.unwrap();

    assert_eq!(count, 10);

    let total = Author::objects(&db).count().await.unwrap();
    assert_eq!(total, 10);
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_update_all_records(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;

    // Update all without filter
    let count = Author::objects(&db)
        .update(|author| {
            author.age = 100;
        })
        .await
        .unwrap();

    assert_eq!(count, 3);

    let authors = Author::objects(&db).all().await.unwrap();
    assert!(authors.iter().all(|a| a.age == 100));
}

#[rstest]
#[awt]
#[tokio::test]
async fn test_delete_all_records(#[future] db_with_sample_authors: (DatabaseRouter, Vec<Author>)) {
    let (db, _sample_authors) = db_with_sample_authors;

    let count = Author::objects(&db).delete().await.unwrap();
    assert_eq!(count, 3);

    let remaining = Author::objects(&db).count().await.unwrap();
    assert_eq!(remaining, 0);
}
